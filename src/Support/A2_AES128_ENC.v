/* *******************************************************************************************************************************

   Copyright © 2015..2024 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         :  A2 AES

   Description          :  AES-128 Encrypt

*********************************************************************************************************************************

   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
   WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS BE LIABLE
   FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT
   OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
   LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
   USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

*********************************************************************************************************************************


********************************************************************************************************************************/

`ifdef  synthesis //JLPL
   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh"
`else
   `include "A2_project_settings.vh"
`endif

module A2_AES128_ENC (
   // Output Interface
   output reg  [143:0]  CTo,           // CupherText                    144 = { CTop[15:0],  CTo[127:0]}
   output reg           CTor,          // Cypher Text Output Ready
   input  wire          CTrd,          // Cypher Text Output Take (pop)
   // Input Interface
   output wire          PKir,          // Plain Text & Key Output Take  (ready to receive)
   input  wire          PKwr,          // Plain Text & Key Input  Give  (ready to send   )
   input  wire [143:0]  PTi,           // Plain  Text Input             144 = { PTip[15:0],  PTi[127:0]}
   input  wire [143:0]  KYi,           // Cypher iKEY Input             144 = {iKEYp[15:0], iKEY[127:0]}
   // Global
   output wire          error,
   input  wire          abort,         // Abandon this encode and go to idle
   input  wire          reset,
   input  wire          clock
   );

// Decollate --------------------------------------------------------------------------------------------------------------------

wire [ 15:0]  PTip;          // Parity bits on Plain Text input
wire [ 15:0]  KYip;          // Parity bits on Cypher key

assign    PTip[15:0] =  PTi[143:128];
assign    KYip[15:0] =  KYi[143:128];

genvar i;
// Constants ====================================================================================================================

// Row constants per Round
localparam  [87:0]  RCON               = {8'h36,8'h1b,8'h80,8'h40,8'h20,8'h10,8'h08,8'h04,8'h02,8'h01,8'h00};
// Parity of row constants per Round
// including start value
localparam  [10:0]  RCONp              = 11'h601;
// Inferred parity of the output data
// given the input address
localparam  [255:0] SBOX_data_parity   = 256'h4f406659427877ea13d164ec9ed304d2e88272648d6f80d17af832b5daff29cd;
// Inferred parity of the input address
// data given the sbox output
localparam  [255:0] SBOX_addr_parity   = 256'hff2b6e4979099b1a573bf3590b5f340d0e749a8ba02d2d0560daf5122acc7476;

// Locals =======================================================================================================================

// Physical Registers
reg     [3:0] ROUND;             // Round Counter
reg   [255:0] PPL;               // Pipeline Registers
reg   [ 31:0] PPLp;              // Pipeline Registers parities
reg           ERR;               // Parity failure capture

// Nets
wire  [255:0] ppl;               // Collated input to pipeline
wire   [31:0] pplp;              // Collated input to pipeline parity
wire  [127:0] keys, perm;        // Key and permute results
wire  [127:0] knx,  pnx;         // Key and permute new or feedback into ppl
wire   [31:0] W0,W1,W2,W3;       // Expanded iKEYs out of pipeline
wire   [31:0] k0,k1,k2,k3;
wire   [31:0] subword;
wire   [31:0] rcon;
//                row  col
wire    [7:0] ST [0:3][0:3];     // State at start of round out of pipeline
wire    [7:0] sb [0:3][0:3];     // State after 'SubBytes()'
wire    [7:0] nx [0:3][0:3];     // State after 'AddRoundiKEY()'
wire    [7:0] con;
wire          conp,idle,last,step,load;

// Parities
wire   [15:0] keysp, permp;
wire   [15:0] pnxp, knxp;
wire   [ 3:0] W0p, W1p, W2p, W3p;
wire          STp [0:3][0:3];
wire          nxp [0:3][0:3];
wire   [ 3:0] swp;
wire   [15:0] stp, sbp, sbdp, sbap;
wire   [ 3:0] subword_data_parity, subword_addr_parity,subword_addr_reorder;
wire   [ 3:0] rconp, k0p, k1p, k2p, k3p;

assign
   idle        =  ROUND == 4'd0,
   last        =  ROUND == 4'd10,
   PKir        =  idle  |  last,
   step        = ~last  & ~idle ,
   load        = (idle  |  last ) & PKwr;

always @(posedge clock)
   if (reset || abort)     ROUND <= `tCQ 4'd 0;
   else if (load)          ROUND <= `tCQ 4'd 1;
   else if (step)          ROUND <= `tCQ 4'd 1 + ROUND;
   else if (last)          ROUND <= `tCQ 4'd 0;

always @(posedge clock)
   if (reset || abort)     CTor  <= `tCQ 1'b0;
   else if (last)          CTor  <= `tCQ 1'b1;
   else if (CTor && CTrd)  CTor  <= `tCQ 1'b0;

// Input & Feedback =============================================================================================================

// Select input or feedback and add round keys
assign   knx[127:0]  =  load  ?               KYi[127:0] :                keys[127:0],
         pnx[127:0]  =  load  ? PTi[127:0]  ^ KYi[127:0] : perm[127:0] ^  keys[127:0];

assign   knxp[15:0]  =  load  ?               KYip[15:0] :                keysp[15:0],
         pnxp[15:0]  =  load  ? PTip[15:0] ~^ KYip[15:0] : permp[15:0] ~^ keysp[15:0];

wire [15:0] check_pnx, check_knx;
for (i=0; i<16; i=i+1) begin : kpnx
   assign check_pnx[i] = ~^pnx[i*8+:8];
   assign check_knx[i] = ~^knx[i*8+:8];
   end

wire check_pnx_ok =  check_pnx == pnxp;
wire check_knx_ok =  check_knx == knxp;

// Initial Permutation (AddRoundKEY)
// Collate into pipeline

assign   ppl[255:0]  =  {pnx[127:0], knx[127:0]},
         pplp[31:0]  =  {pnxp[15:0], knxp[15:0]};

always @(posedge clock) if (load | step) PPL[255:0]  <= `tCQ ppl[255:0];
always @(posedge clock) if (load | step) PPLp[31:0]  <= `tCQ pplp[31:0];

wire [31:0] check_PPL;
for (i=0; i<32; i=i+1) begin : cppl
   assign check_PPL[i] = ~^PPL[i*8+:8];                           // Create parity on output of pipeline
   end
wire check_PPL_ok =  check_PPL == PPLp;                           // Check recreated against expected

// Decollate out of pipeline; load state matrix and words 0..3
assign
   {ST[0][0],ST[1][0],ST[2][0],ST[3][0],
    ST[0][1],ST[1][1],ST[2][1],ST[3][1],
    ST[0][2],ST[1][2],ST[2][2],ST[3][2],
    ST[0][3],ST[1][3],ST[2][3],ST[3][3],
    W0[31:0],W1[31:0],W2[31:0],W3[31:0]}  =  PPL[255:0];
    // NOTE: Big Endian Byte ordering of input data to column major state matrix

assign
   {STp[0][0],STp[1][0],STp[2][0],STp[3][0],
    STp[0][1],STp[1][1],STp[2][1],STp[3][1],
    STp[0][2],STp[1][2],STp[2][2],STp[3][2],
    STp[0][3],STp[1][3],STp[2][3],STp[3][3],
    W0p[3:0],W1p[3:0],W2p[3:0],W3p[3:0]}  =  PPLp[31:0];

wire [15:0] check_ST, stparity;
wire [ 3:0] check_W0,check_W1,check_W2,check_W3;

for (i=0; i<16; i=i+1) begin : STs
   assign check_ST[i]      = ~^ST[ i[3:2]][i[1:0]];
   assign stparity[i]      =   STp[i[3:2]][i[1:0]];
   end
for (i=0; i<4; i=i+1) begin : Ws
   assign check_W0[i]   = ~^W0[i*8+:8];
   assign check_W1[i]   = ~^W1[i*8+:8];
   assign check_W2[i]   = ~^W2[i*8+:8];
   assign check_W3[i]   = ~^W3[i*8+:8];
   end

wire check_ST_ok  =  check_ST  == stparity;
wire check_W0_ok  =  check_W0  == W0p;
wire check_W1_ok  =  check_W1  == W1p;
wire check_W2_ok  =  check_W2  == W2p;
wire check_W3_ok  =  check_W3  == W3p;

// Key Expansion ================================================================================================================

for (i=0; i<4; i=i+1) begin : isbsw
   sbox isb (
      .p(swp[i]),
      .y(subword[8*i+:8]),
      .a(W3[8*((i+3)%4)+:8])
      );
   assign subword_data_parity[i] = SBOX_data_parity[W3[8*((i+3)%4)+:8]];
   assign subword_addr_parity[i] = SBOX_addr_parity[subword[8*i+:8]];
   end

// Preserve parity through sboxes
// predicated address parity should match STp
assign
   subword_addr_reorder =  {W3p[2:0],W3p[3]},
   con[ 7:0]            =  RCON[8* ROUND[3:0] +: 8],
   conp                 =  RCONp[  ROUND[3:0]],
   rcon[31:0]           = {con[7:0]  ^ W0[31:24], W0[23:0]},
   rconp[3:0]           = {    conp ~^ W0p[3],    W0p[2:0]};

wire check_SubKeys_ok   = (subword_addr_reorder == subword_addr_parity) & (swp == subword_data_parity);
wire check_con_ok       =  ~^con[7:0]    == conp;
wire check_rcon_ok      =  ~^rcon[31:24] == rconp[3];

// Expand
assign
   k0[31:0]    =  subword[31:0] ^ rcon[31:0],
   k1[31:0]    =  subword[31:0] ^ rcon[31:0] ^ W1[31:0],
   k2[31:0]    =  subword[31:0] ^ rcon[31:0] ^ W1[31:0] ^ W2[31:0],
   k3[31:0]    =  subword[31:0] ^ rcon[31:0] ^ W1[31:0] ^ W2[31:0] ^ W3[31:0];

// Parity Expand
assign
   k0p[3:0]    =  swp[3:0] ~^ rconp[3:0],
   k1p[3:0]    =  swp[3:0] ~^ rconp[3:0] ~^ W1p[3:0],
   k2p[3:0]    =  swp[3:0] ~^ rconp[3:0] ~^ W1p[3:0] ~^ W2p[3:0],
   k3p[3:0]    =  swp[3:0] ~^ rconp[3:0] ~^ W1p[3:0] ~^ W2p[3:0] ~^ W3p[3:0];

wire [3:0]  check_k0p,check_k1p,check_k2p,check_k3p;
for (i=0; i<4; i=i+1) begin : ks
   assign check_k0p[i] = ~^k0[8*i+:8];
   assign check_k1p[i] = ~^k1[8*i+:8];
   assign check_k2p[i] = ~^k2[8*i+:8];
   assign check_k3p[i] = ~^k3[8*i+:8];
   end
wire check_k0p_ok =  check_k0p == k0p;
wire check_k1p_ok =  check_k1p == k1p;
wire check_k2p_ok =  check_k2p == k2p;
wire check_k3p_ok =  check_k3p == k3p;

// Feedback
assign
   keys[127:0] =  {k0, k1, k2, k3 },
   keysp[15:0] =  {k0p,k1p,k2p,k3p};

// Round Permutations ===========================================================================================================

// Preserve parity through sboxes
assign
   stp[15:0]  = {STp[3][3],STp[3][2],STp[3][1],STp[3][0],STp[2][3],STp[2][2],STp[2][1],STp[2][0],
                 STp[1][3],STp[1][2],STp[1][1],STp[1][0],STp[0][3],STp[0][2],STp[0][1],STp[0][0]};

for (i=0; i<16; i=i+1) begin : isbdp
   sbox isb (
      .p(sbp[i]),
      .y(sb[i[3:2]][i[1:0]]),
      .a(ST[i[3:2]][i[1:0]])
      );
   assign sbdp[i]  =  SBOX_data_parity[ST[i[3:2]][i[1:0]]];
   assign sbap[i]  =  SBOX_addr_parity[sb[i[3:2]][i[1:0]]];
   end

// predicted data parity should match sbp AND predicated address parity should match STp
wire  check_SubBytes_ok   = (stp == sbap) & (sbp == sbdp);

// Shift Rows, MixColummns
assign // 4 columns
   {nx[0][0], nx[1][0], nx[2][0], nx[3][0]}  = mix_col(sb[0][0],sb[1][1],sb[2][2],sb[3][3]), // & 8'h40),
   {nx[0][1], nx[1][1], nx[2][1], nx[3][1]}  = mix_col(sb[0][1],sb[1][2],sb[2][3],sb[3][0]),
   {nx[0][2], nx[1][2], nx[2][2], nx[3][2]}  = mix_col(sb[0][2],sb[1][3],sb[2][0],sb[3][1]),
   {nx[0][3], nx[1][3], nx[2][3], nx[3][3]}  = mix_col(sb[0][3],sb[1][0],sb[2][1],sb[3][2]);

// Shift parity the same way
wire  [15:0] msbs = {sb[3][3][7],sb[3][2][7],sb[3][1][7],sb[3][0][7], sb[2][3][7],sb[2][2][7],sb[2][1][7],sb[2][0][7],
                     sb[1][3][7],sb[1][2][7],sb[1][1][7],sb[1][0][7], sb[0][3][7],sb[0][2][7],sb[0][1][7],sb[0][0][7]};
assign // 4 columns
   {nxp[0][0],nxp[1][0],nxp[2][0],nxp[3][0]} = mix_col_parity(msbs[0],msbs[5],msbs[10],msbs[15],sbdp[0],sbdp[5],sbdp[10],sbdp[15]),
   {nxp[0][1],nxp[1][1],nxp[2][1],nxp[3][1]} = mix_col_parity(msbs[1],msbs[6],msbs[11],msbs[12],sbdp[1],sbdp[6],sbdp[11],sbdp[12]),
   {nxp[0][2],nxp[1][2],nxp[2][2],nxp[3][2]} = mix_col_parity(msbs[2],msbs[7],msbs[08],msbs[13],sbdp[2],sbdp[7],sbdp[08],sbdp[13]),
   {nxp[0][3],nxp[1][3],nxp[2][3],nxp[3][3]} = mix_col_parity(msbs[3],msbs[4],msbs[09],msbs[14],sbdp[3],sbdp[4],sbdp[09],sbdp[14]);

// feedback
assign
   perm[127:0] =  {nx[0][0], nx[1][0], nx[2][0], nx[3][0],
                   nx[0][1], nx[1][1], nx[2][1], nx[3][1],
                   nx[0][2], nx[1][2], nx[2][2], nx[3][2],
                   nx[0][3], nx[1][3], nx[2][3], nx[3][3]},

// Pass parity back around too
   permp[15:0] =  {nxp[0][0], nxp[1][0], nxp[2][0], nxp[3][0],
                   nxp[0][1], nxp[1][1], nxp[2][1], nxp[3][1],
                   nxp[0][2], nxp[1][2], nxp[2][2], nxp[3][2],
                   nxp[0][3], nxp[1][3], nxp[2][3], nxp[3][3]};

wire [15:0]  check_perm;
for (i=0; i<16; i=i+1) begin : cp
   assign check_perm[i] = ~^perm[8*i+:8];
   end
wire check_perm_ok =  check_perm == permp;

// Final text output ============================================================================================================

// ShiftRows & AddRoundiKEY
always @(posedge clock) if (last) begin
   CTo[143:140] <= `tCQ  { sbp[0],   sbp[5],   sbp[10],  sbp[15] } ~^ k0p[3:0];
   CTo[139:136] <= `tCQ  { sbp[1],   sbp[6],   sbp[11],  sbp[12] } ~^ k1p[3:0];
   CTo[135:132] <= `tCQ  { sbp[2],   sbp[7],   sbp[08],  sbp[13] } ~^ k2p[3:0];
   CTo[131:128] <= `tCQ  { sbp[3],   sbp[4],   sbp[09],  sbp[14] } ~^ k3p[3:0];
   CTo[127:96 ] <= `tCQ  { sb[0][0], sb[1][1], sb[2][2], sb[3][3]}  ^ k0[31:0];
   CTo[ 95:64 ] <= `tCQ  { sb[0][1], sb[1][2], sb[2][3], sb[3][0]}  ^ k1[31:0];
   CTo[ 63:32 ] <= `tCQ  { sb[0][2], sb[1][3], sb[2][0], sb[3][1]}  ^ k2[31:0];
   CTo[ 31:0  ] <= `tCQ  { sb[0][3], sb[1][0], sb[2][1], sb[3][2]}  ^ k3[31:0];
   end

// Parity check!
wire  [15:0]   parity_check_CTo;

for (i=0;i<16;i=i+1)
   assign parity_check_CTo[i] = ~^(CTo[8*i+:8]);

wire  check_CTo_ok   = ~idle | (parity_check_CTo[15:0] == CTo[143:128]);

// Gather parity checks
wire  parity_ok      =  check_pnx_ok
                     &  check_knx_ok
                     &  check_PPL_ok
                     &  check_ST_ok
                     &  check_W0_ok
                     &  check_W1_ok
                     &  check_W2_ok
                     &  check_W3_ok
                     &  check_k0p_ok
                     &  check_k1p_ok
                     &  check_k2p_ok
                     &  check_k3p_ok
                     &  check_SubKeys_ok
                     &  check_con_ok
                     &  check_rcon_ok
                     &  check_SubBytes_ok
                     &  check_perm_ok
                     &  check_CTo_ok;

always @(posedge clock)
   if (reset || abort)     ERR   <= `tCQ 1'b 1;
   else if (load)          ERR   <= `tCQ 1'b 0;
   else if (step || last)  ERR   <= `tCQ ERR | ~parity_ok;

assign   error    = ERR;

// Cypher Mix Columns Function ==================================================================================================

function [31:0] mix_col;
input     [7:0] c0,  c1,  c2,  c3;
reg       [7:0] c0x2,c1x2,c2x2,c3x2;
   begin
   c0x2[7:0]      = {c0[6:4], c0[7] ^ c0[3], c0[7] ^ c0[2], c0[1], c0[0] ^ c0[7], c0[7]}; //  Multiply by 2 (GF2) modulo 0x1b
   c1x2[7:0]      = {c1[6:4], c1[7] ^ c1[3], c1[7] ^ c1[2], c1[1], c1[0] ^ c1[7], c1[7]}; //  xtime() from fips-197
   c2x2[7:0]      = {c2[6:4], c2[7] ^ c2[3], c2[7] ^ c2[2], c2[1], c2[0] ^ c2[7], c2[7]}; // (cx << 1) ^ ({8{cx[7]}} & 8'h1b)
   c3x2[7:0]      = {c3[6:4], c3[7] ^ c3[3], c3[7] ^ c3[2], c3[1], c3[0] ^ c3[7], c3[7]};

   mix_col[31:24] =  c0x2     ^ (c1x2^c1) ^  c2       ^  c3;                              // 2 3 1 1  Sum the terms
   mix_col[23:16] =  c0       ^  c1x2     ^ (c2x2^c2) ^  c3;                              // 1 2 3 1
   mix_col[15:08] =  c0       ^  c1       ^  c2x2     ^ (c3x2^c3);                        // 1 1 2 3
   mix_col[07:00] = (c0x2^c0) ^  c1       ^  c2       ^  c3x2;                            // 3 1 1 2
   end
endfunction

function  [3:0] mix_col_parity;
input           c0_7, c1_7, c2_7, c3_7;
input           c0p,  c1p,  c2p,  c3p;
reg             c0x2p,c1x2p,c2x2p,c3x2p;
   begin
   c0x2p = c0_7 ^ c0p;
   c1x2p = c1_7 ^ c1p;
   c2x2p = c2_7 ^ c2p;
   c3x2p = c3_7 ^ c3p;

   mix_col_parity[3] =  c0x2p ^ c1x2p ^ c1p ^ c2p ^ c3p;
   mix_col_parity[2] =  c2x2p ^ c1x2p ^ c0p ^ c2p ^ c3p;
   mix_col_parity[1] =  c2x2p ^ c3x2p ^ c0p ^ c1p ^ c3p;
   mix_col_parity[0] =  c0x2p ^ c3x2p ^ c0p ^ c1p ^ c2p;
   end
endfunction

endmodule

// Gate-level SBOX ==============================================================================================================
// Transposed to little endian from Figure 7 of the NIST publication fips-197
// localparam  [2048-1:0]  SBOX = {                                                                                      upper
// lower address nibble (y)  <---                                                                                        address
//     f      e      d      c      b      a      9      8      7      6      5      4      3      2      1      0        nibble
// 8'h16, 8'hbb, 8'h54, 8'hb0, 8'h0f, 8'h2d, 8'h99, 8'h41, 8'h68, 8'h42, 8'he6, 8'hbf, 8'h0d, 8'h89, 8'ha1, 8'h8c,       f  (x)
// 8'hdf, 8'h28, 8'h55, 8'hce, 8'he9, 8'h87, 8'h1e, 8'h9b, 8'h94, 8'h8e, 8'hd9, 8'h69, 8'h11, 8'h98, 8'hf8, 8'he1,       e
// 8'h9e, 8'h1d, 8'hc1, 8'h86, 8'hb9, 8'h57, 8'h35, 8'h61, 8'h0e, 8'hf6, 8'h03, 8'h48, 8'h66, 8'hb5, 8'h3e, 8'h70,       d  ^
// 8'h8a, 8'h8b, 8'hbd, 8'h4b, 8'h1f, 8'h74, 8'hdd, 8'he8, 8'hc6, 8'hb4, 8'ha6, 8'h1c, 8'h2e, 8'h25, 8'h78, 8'hba,       c  |
//                                                                                                                          |
// 8'h08, 8'hae, 8'h7a, 8'h65, 8'hea, 8'hf4, 8'h56, 8'h6c, 8'ha9, 8'h4e, 8'hd5, 8'h8d, 8'h6d, 8'h37, 8'hc8, 8'he7,       b
// 8'h79, 8'he4, 8'h95, 8'h91, 8'h62, 8'hac, 8'hd3, 8'hc2, 8'h5c, 8'h24, 8'h06, 8'h49, 8'h0a, 8'h3a, 8'h32, 8'he0,       a
// 8'hdb, 8'h0b, 8'h5e, 8'hde, 8'h14, 8'hb8, 8'hee, 8'h46, 8'h88, 8'h90, 8'h2a, 8'h22, 8'hdc, 8'h4f, 8'h81, 8'h60,       9
// 8'h73, 8'h19, 8'h5d, 8'h64, 8'h3d, 8'h7e, 8'ha7, 8'hc4, 8'h17, 8'h44, 8'h97, 8'h5f, 8'hec, 8'h13, 8'h0c, 8'hcd,       8
//
// 8'hd2, 8'hf3, 8'hff, 8'h10, 8'h21, 8'hda, 8'hb6, 8'hbc, 8'hf5, 8'h38, 8'h9d, 8'h92, 8'h8f, 8'h40, 8'ha3, 8'h51,       7
// 8'ha8, 8'h9f, 8'h3c, 8'h50, 8'h7f, 8'h02, 8'hf9, 8'h45, 8'h85, 8'h33, 8'h4d, 8'h43, 8'hfb, 8'haa, 8'hef, 8'hd0,       6
// 8'hcf, 8'h58, 8'h4c, 8'h4a, 8'h39, 8'hbe, 8'hcb, 8'h6a, 8'h5b, 8'hb1, 8'hfc, 8'h20, 8'hed, 8'h00, 8'hd1, 8'h53,       5
// 8'h84, 8'h2f, 8'he3, 8'h29, 8'hb3, 8'hd6, 8'h3b, 8'h52, 8'ha0, 8'h5a, 8'h6e, 8'h1b, 8'h1a, 8'h2c, 8'h83, 8'h09,       4
//
// 8'h75, 8'hb2, 8'h27, 8'heb, 8'he2, 8'h80, 8'h12, 8'h07, 8'h9a, 8'h05, 8'h96, 8'h18, 8'hc3, 8'h23, 8'hc7, 8'h04,       3
// 8'h15, 8'h31, 8'hd8, 8'h71, 8'hf1, 8'he5, 8'ha5, 8'h34, 8'hcc, 8'hf7, 8'h3f, 8'h36, 8'h26, 8'h93, 8'hfd, 8'hb7,       2
// 8'hc0, 8'h72, 8'ha4, 8'h9c, 8'haf, 8'ha2, 8'hd4, 8'had, 8'hf0, 8'h47, 8'h59, 8'hfa, 8'h7d, 8'hc9, 8'h82, 8'hca,       1
// 8'h76, 8'hab, 8'hd7, 8'hfe, 8'h2b, 8'h67, 8'h01, 8'h30, 8'hc5, 8'h6f, 8'h6b, 8'hf2, 8'h7b, 8'h77, 8'h7c, 8'h63 };     0
//                                                                                                          LS entry
// Reduced to approximately 128 gates...

module sbox (
   output wire       p,
   output wire [7:0] y,
   input  wire [7:0] a
   );
wire
   u7  = a[0], u6 = a[1], u5 = a[2], u4 = a[3], u3 = a[4], u2 = a[5], u1 = a[6], u0 = a[7],

   t1  =  u0 ^ u3,   t2  =  u0 ^ u5,   t3  = u0  ^ u6,   t4  = u3  ^ u5,   t5  = u4  ^ u6,   t6  = t1  ^ t5,   t7  = u1  ^ u2,
   t8  =  u7 ^ t6,   t9  =  u7 ^ t7,   t10 = t6  ^ t7,   t11 = u1  ^ u5,   t12 = u2  ^ u5,   t13 = t3  ^ t4,   t14 = t6  ^ t11,
   t15 =  t5 ^ t11,  t16 =  t5 ^ t12,  t17 = t9  ^ t16,  t18 = u3  ^ u7,   t19 = t7  ^ t18,  t20 = t1  ^ t19,  t21 = u6  ^ u7,
   t22 =  t7 ^ t21,  t23 =  t2 ^ t22,  t24 = t2  ^ t10,  t25 = t20 ^ t17,  t26 = t3  ^ t16,  t27 = t1  ^ t12,

   m1  = t13 & t6,   m2  = t23 & t8,   m3  = t14 ^ m1,   m4  = t19 & u7,   m5  = m4  ^ m1,   m6  = t3  & t16,  m7  = t22 & t9,
   m8  = t26 ^ m6,   m9  = t20 & t17,  m10 = m9  ^ m6,   m11 = t1  & t15,  m12 = t4  & t27,  m13 = m12 ^ m11,  m14 = t2  & t10,
   m15 = m14 ^ m11,  m16 = m3  ^ m2,   m17 = m5  ^ t24,  m18 = m8  ^ m7,   m19 = m10 ^ m15,  m20 = m16 ^ m13,  m21 = m17 ^ m15,
   m22 = m18 ^ m13,  m23 = m19 ^ t25,  m24 = m22 ^ m23,  m25 = m22 & m20,  m26 = m21 ^ m25,  m27 = m20 ^ m21,  m28 = m23 ^ m25,
   m29 = m28 & m27,  m30 = m26 & m24,  m31 = m20 & m23,  m32 = m27 & m31,  m33 = m27 ^ m25,  m34 = m21 & m22,  m35 = m24 & m34,
   m36 = m24 ^ m25,  m37 = m21 ^ m29,  m38 = m32 ^ m33,  m39 = m23 ^ m30,  m40 = m35 ^ m36,  m41 = m38 ^ m40,  m42 = m37 ^ m39,
   m43 = m37 ^ m38,  m44 = m39 ^ m40,  m45 = m42 ^ m41,  m46 = m44 & t6,   m47 = m40 & t8,   m48 = m39 & u7,   m49 = m43 & t16,
   m50 = m38 & t9,   m51 = m37 & t17,  m52 = m42 & t15,  m53 = m45 & t27,  m54 = m41 & t10,  m55 = m44 & t13,  m56 = m40 & t23,
   m57 = m39 & t19,  m58 = m43 & t3,   m59 = m38 & t22,  m60 = m37 & t20,  m61 = m42 & t1,   m62 = m45 & t4,   m63 = m41 & t2,

   p0  = m61 ^ m62,  p1  = m50 ^ m56,  p2  = m46 ^ m48,  p3  = m47 ^ m55,  p4  = m54 ^ m58,  p5  = m49 ^ m61,  p6  = m62 ^ p5,
   p7  = m46 ^ p3,   p8  = m51 ^ m59,  p9  = m52 ^ m53,  p10 = m53 ^ p4,   p11 = m60 ^ p2,   p12 = m48 ^ m51,  p13 = m50 ^ p0,
   p14 = m52 ^ m61,  p15 = m55 ^ p1,   p16 = m56 ^ p0,   p17 = m57 ^ p1,   p18 = m58 ^ p8,   p19 = m63 ^ p4,   p20 = p0  ^ p1,
   p21 = p1  ^ p7,   p22 = p3  ^ p12,  p23 = p18 ^ p2,   p24 = p15 ^ p9,   p25 = p6  ^ p10,  p26 = p7  ^ p9,   p27 = p8  ^ p10,
   p28 = p11 ^ p14,  p29 = p11 ^ p17,

   s0  = p6  ^ p24,  s1  = p16 ^ p26,  s2  = p19 ^ p28,  s3  = p6  ^ p21,
   s4  = p20 ^ p22,  s5  = p25 ^ p29,  s6  = p13 ^ p27,  s7  = p6  ^ p23;

assign
   y   =  {s0,~s1,~s2,s3,s4,s5,~s6,~s7},
   p   = ~^y[7:0];

endmodule

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
`ifdef   A2_AES128_ENC_verify

`timescale 1ns/1ps

module   t;

// Output Interface
wire  [127:0] CTo;     // Cypher Text Output (Big Endian ordered bytes)
wire          ct_gv;   // Cypher Text Output Give (test available)
reg           ct_tk;   // Cypher Text Output Take (pop)
// Input Interface
wire          pt_tk;   // Plain  Text Input  Take  (ready to receive)
reg           pt_gv;   // Plain  Text Input  Give  (push)
reg   [127:0] PTi;     // Plain  Text Input  (Big Endian ordered bytes)
reg   [127:0] iKEY;    // Cypher iKEY Input
// Global
wire          CTerr;
reg           abort;
reg           reset;
reg           clock;

   // Transposed to little endian from Figure 7 of the NIST publication fips-197
localparam  [2048-1:0]  SBOX = {                                                                                 // upper
// lower address nibble (y)  <---                                                                                   // address
//     f      e      d      c      b      a      9      8      7      6      5      4      3      2      1      0   // nibble
   8'h16, 8'hbb, 8'h54, 8'hb0, 8'h0f, 8'h2d, 8'h99, 8'h41, 8'h68, 8'h42, 8'he6, 8'hbf, 8'h0d, 8'h89, 8'ha1, 8'h8c,  // f  (x)
   8'hdf, 8'h28, 8'h55, 8'hce, 8'he9, 8'h87, 8'h1e, 8'h9b, 8'h94, 8'h8e, 8'hd9, 8'h69, 8'h11, 8'h98, 8'hf8, 8'he1,  // e
   8'h9e, 8'h1d, 8'hc1, 8'h86, 8'hb9, 8'h57, 8'h35, 8'h61, 8'h0e, 8'hf6, 8'h03, 8'h48, 8'h66, 8'hb5, 8'h3e, 8'h70,  // d  ^
   8'h8a, 8'h8b, 8'hbd, 8'h4b, 8'h1f, 8'h74, 8'hdd, 8'he8, 8'hc6, 8'hb4, 8'ha6, 8'h1c, 8'h2e, 8'h25, 8'h78, 8'hba,  // c  |
                                                                                                                    //    |
   8'h08, 8'hae, 8'h7a, 8'h65, 8'hea, 8'hf4, 8'h56, 8'h6c, 8'ha9, 8'h4e, 8'hd5, 8'h8d, 8'h6d, 8'h37, 8'hc8, 8'he7,  // b
   8'h79, 8'he4, 8'h95, 8'h91, 8'h62, 8'hac, 8'hd3, 8'hc2, 8'h5c, 8'h24, 8'h06, 8'h49, 8'h0a, 8'h3a, 8'h32, 8'he0,  // a
   8'hdb, 8'h0b, 8'h5e, 8'hde, 8'h14, 8'hb8, 8'hee, 8'h46, 8'h88, 8'h90, 8'h2a, 8'h22, 8'hdc, 8'h4f, 8'h81, 8'h60,  // 9
   8'h73, 8'h19, 8'h5d, 8'h64, 8'h3d, 8'h7e, 8'ha7, 8'hc4, 8'h17, 8'h44, 8'h97, 8'h5f, 8'hec, 8'h13, 8'h0c, 8'hcd,  // 8
                                                                                                                    //
   8'hd2, 8'hf3, 8'hff, 8'h10, 8'h21, 8'hda, 8'hb6, 8'hbc, 8'hf5, 8'h38, 8'h9d, 8'h92, 8'h8f, 8'h40, 8'ha3, 8'h51,  // 7
   8'ha8, 8'h9f, 8'h3c, 8'h50, 8'h7f, 8'h02, 8'hf9, 8'h45, 8'h85, 8'h33, 8'h4d, 8'h43, 8'hfb, 8'haa, 8'hef, 8'hd0,  // 6
   8'hcf, 8'h58, 8'h4c, 8'h4a, 8'h39, 8'hbe, 8'hcb, 8'h6a, 8'h5b, 8'hb1, 8'hfc, 8'h20, 8'hed, 8'h00, 8'hd1, 8'h53,  // 5
   8'h84, 8'h2f, 8'he3, 8'h29, 8'hb3, 8'hd6, 8'h3b, 8'h52, 8'ha0, 8'h5a, 8'h6e, 8'h1b, 8'h1a, 8'h2c, 8'h83, 8'h09,  // 4
                                                                                                                    //
   8'h75, 8'hb2, 8'h27, 8'heb, 8'he2, 8'h80, 8'h12, 8'h07, 8'h9a, 8'h05, 8'h96, 8'h18, 8'hc3, 8'h23, 8'hc7, 8'h04,  // 3
   8'h15, 8'h31, 8'hd8, 8'h71, 8'hf1, 8'he5, 8'ha5, 8'h34, 8'hcc, 8'hf7, 8'h3f, 8'h36, 8'h26, 8'h93, 8'hfd, 8'hb7,  // 2
   8'hc0, 8'h72, 8'ha4, 8'h9c, 8'haf, 8'ha2, 8'hd4, 8'had, 8'hf0, 8'h47, 8'h59, 8'hfa, 8'h7d, 8'hc9, 8'h82, 8'hca,  // 1
   8'h76, 8'hab, 8'hd7, 8'hfe, 8'h2b, 8'h67, 8'h01, 8'h30, 8'hc5, 8'h6f, 8'h6b, 8'hf2, 8'h7b, 8'h77, 8'h7c, 8'h63 };// 0

//                                                                                                       LS entry

integer i,j;
reg   [255:0] parity;
reg   [  7:0] temp;
reg   [ 15:0] PTip, iKEYp;
reg   [ 10:0] rcp;
wire  [ 15:0] CTop;

initial begin
   parity = 256'h0;
   for (i=0; i<256; i=i+1) begin
      j=0;
      while (SBOX[8*j+:8] != i[7:0])
         j=j+1;
      // i = sbox entry j = address
      $display(" sbox entry to address parity: entry = 8'h%2h address = 8'h%2h address parity = %b",i[7:0],j[7:0],~^(j[7:0]));
      parity = {~^(j[7:0]),parity[255:1]};
      end
   $display("\n\n");
   $display(" sbox address parity vector = 256'h%h\n\n",parity);

   for (i=0; i<256; i=i+1)
      parity[i] = ~^SBOX[8*i+:8];
   $display(" sbox  entry  parity vector = 256'h%h\n\n",parity);
   for (i=0; i<11; i=i+1)
      rcp[i] = ~^mut.RCON[8*i+:8];
   $display(" rcon   parity   vector     = 256'h%h\n\n",rcp);

   for (i=0; i<256; i=i+1) begin
      temp = {i[6:4], i[7] ^ i[3], i[7] ^ i[2], i[1], i[0] ^ i[7], i[7]};
      $display(" input %h pin %b output %h pout %b parity difference %b", i[7:0],~^i[7:0],temp,~^temp[7:0], ~^i[7:0] ^ ~^temp[7:0] );
      end

   for (i=0; i<256; i=i+1)
      $display(" Parity of 2 4-bit fields f0= %h f1= %h p0= %b p1= %b XOR= %h xp= %b  p0~^p1== %b diff= %b",
                       i[7:4],
                       i[3:0],
                     ~^i[7:4],
                     ~^i[3:0],
                       i[7:4]^i[3:0],
                    ~^(i[7:4]^i[3:0]),
                    (~^i[7:4])~^(~^i[3:0]),
                   ((~^i[7:4])~^(~^i[3:0])==(~^(i[7:4]^i[3:0]))) );

   end

function [15:0] parity_gen;
input   [127:0] d;
integer i;
for (i=0;i<16;i=i+1)
   parity_gen[i] = ~^d[8*i+:8];
endfunction

A2_AES128_ENC mut (
   .CTo   ({CTop[15:0],  CTo[127:0]}),
   .CTor  (ct_gv),
   .CTrd  (ct_tk),
   .PTi   ({ PTip[15:0],  PTi[127:0]}),
   .KYi   ({iKEYp[15:0], iKEY[127:0]}),
   .PKir  (pt_tk),
   .PKwr  (pt_gv),
   .error (CTerr),
   .abort (abort),
   .reset (reset),
   .clock (clock)
   );

initial begin
   clock =  1'b0;
   forever begin
      #10 clock = ~clock;
      end
   end

wire  [3:0] rcon_index1 =  mut.ROUND[3:0];

initial begin
   reset     =  1'b0;
   $display("     reset = 0");
   abort     =  1'b0;
   $display("     abort = 0");
   pt_gv     =  1'b0;
   $display("     pt_gv = 0");
   ct_tk     =  1'b0;
   $display("     ct_tk = 0");
   $display("     clock = %b, time = %t", clock,$time); #5;
   $display("     clock = %b, time = %t", clock,$time); #10;
   $display("     clock = %b, time = %t", clock,$time); #10;

   $display(" Test 1: Simple encode");
   repeat (10)  @(posedge clock) #1;
   reset     =  1'b1;
   repeat (01)  @(posedge clock) #1;
   reset     =  1'b0;
   repeat (10)  @(posedge clock) #1;
   pt_gv     =  1'b1;
   ct_tk     =  1'b1;
   PTi       =  128'h00112233445566778899aabbccddeeff;
   iKEY      =  128'h000102030405060708090a0b0c0d0e0f;
   PTip      =  parity_gen(PTi);
   iKEYp     =  parity_gen(iKEY);
   repeat (01)  @(posedge clock) #1;
   pt_gv     =  1'b0;
   PTi       =  128'hxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx;
   iKEY      =  128'hxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx;
   PTip      =   16'hxxxx;
   iKEYp     =   16'hxxxx;
   repeat (11)  @(posedge clock) #1;
   ct_tk     =  1'b0;
   repeat (10)  @(posedge clock) #1;

   // Try another ------------------------------------
   $display(" Test 2: Try another");
   repeat (10)  @(posedge clock) #1;
   pt_gv     =  1'b1;
   ct_tk     =  1'b1;
   PTi       =  128'h3243f6a8885a308d313198a2e0370734;
   iKEY      =  128'h2b7e151628aed2a6abf7158809cf4f3c;
   PTip      =  parity_gen(PTi);
   iKEYp     =  parity_gen(iKEY);
   repeat (01)  @(posedge clock) #1; pt_gv       =  1'b0;
   PTi       =  128'hxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx;
   iKEY      =  128'hxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx;
   PTip      =   16'hxxxx;
   iKEYp     =   16'hxxxx;
   repeat (11)  @(posedge clock) #1; ct_tk       =  1'b0;
   repeat (10)  @(posedge clock) #1;

   // Try abort --------------------------------------
   $display(" Test 3: Try an abort");

   repeat (10)  @(posedge clock) #1; pt_gv       =  1'b1;
                                     ct_tk       =  1'b1;
   PTi       =  128'h00112233445566778899aabbccddeeff;
   iKEY      =  128'h000102030405060708090a0b0c0d0e0e;   // 1-bit error
   PTip      =  parity_gen(PTi);
   iKEYp     =  parity_gen(iKEY);
   repeat (01)  @(posedge clock) #1; pt_gv       =  1'b0;
   PTi       =  128'hxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx;
   iKEY      =  128'hxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx;
   PTip      =   16'hxxxx;
   iKEYp     =   16'hxxxx;
   repeat (05)  @(posedge clock) #1; abort       =  1'b1;
   repeat (01)  @(posedge clock) #1; abort       =  1'b0;
   repeat (01)  @(posedge clock) #1; pt_gv       =  1'b1;
   PTi       =  128'h00112233445566778899aabbccddeeff;
   iKEY      =  128'h000102030405060708090a0b0c0d0e0f;
   PTip      =  parity_gen(PTi);
   iKEYp     =  parity_gen(iKEY);
   repeat (01)  @(posedge clock) #1; pt_gv       =  1'b0;
   PTi       =  128'hxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx;
   iKEY      =  128'hxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx;
   PTip      =   16'hxxxx;
   iKEYp     =   16'hxxxx;
   repeat (01)  @(posedge clock) #1; pt_gv       =  1'b0;
   repeat (11)  @(posedge clock) #1; ct_tk       =  1'b0;
   repeat (10)  @(posedge clock) #1;

   repeat (100) @(posedge clock) #1;

   $finish;
   end

always @(posedge clock)
   if (ct_gv && ct_tk) begin
      if      (( CTo == 128'h69c4e0d86a7b0430d8cdb78070b4c55a )
            || ( CTo == 128'h3925841d02dc09fbdc118597196a0b32 ))
         $display("\n   Test Successful per fips-197:C1 test case = %h \n",CTo);
      else begin
         $display("\n   Test Failed     per fips-197:C1 test case = %h   ",CTo);
         $display(  "      Should be        fips-197:C1 test case = %h   ",128'h69c4e0d86a7b0430d8cdb78070b4c55a);
         $display(  "      or should be     fips-197    test case = %h \n",128'h3925841d02dc09fbdc118597196a0b32);
         end
      if (CTerr)
         $display("\n   Parity checks failed \n");
      else
         $display("\n   Parity checks passed \n");
      end


endmodule
`endif
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
