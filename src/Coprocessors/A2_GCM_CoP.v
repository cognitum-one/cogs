//===============================================================================================================================
//
// Copyright © 2024 Advanced Architectures
//
// All rights reserved
// Confidential Information
// Limited Distribution to Authorized Persons Only
// Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
// Project Name         : Cognitum
//
// Description          : Galois Counter Mode AES Encryption Co-processor (Pipe-Based I/O)
//
//===============================================================================================================================
// THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
// BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
// TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
// AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//  Requests for  GCM
//    greq :  10000    Session 0 from H
//    greq :  10001    Session 0 from E
//    greq :  10010    Session 0 to   H
//    greq :  10011    Session 0 to   E      Ez distinguished by position and the main state machine
//    greq :  10100    Session 1 from H
//    greq :  10101    Session 1 from E
//    greq :  10110    Session 1 to   H
//    greq :  10111    Session 1 to   E
//    greq :  11000    Multicast
//
//  All I/O primarily through pipes with FIFO-like on condition code bus (cc)
//
//===============================================================================================================================

`timescale 1ps/10fs
`ifndef tCQ
`define tCQ #1
`endif

module A2_GCM_CoP #(
   parameter   [12:0]   BASE     = 13'h0000,  // Base Address of Co-processor
   parameter            NUM_REGS = 32,
   parameter            LNRG     = 5
   )(
   // A2S IO Interface - Processor writes (with pipe for 128-bit registers)
   output wire [32:0]   imiso,      // Processor IO response       33 {IOk, IOo[31:0]}
   input  wire [47:0]   imosi,      // Processor IO request        48 {cIO[2:0], iIO[31:0], aIO[12:0]}

   // Condition codes - Extended for pipe flags
   output wire [ 1:0]   cc,         // [1] equal = Reduction AND of an XOR function, [0] State machine is IDLE (not BUSY)

   // AES Request Interface (5-bit pipe)
   output wire [ 5:0]   ymosi,      //  5 = {gwr, grq[3:0]}                            -- AES request input //JLPL - 10_11_25
   input  wire          ymosi_fb,   //      {irdy}                                                          //JLPL - 10_11_25

   // AES Response Interface (32-bit data, burst of 4, collected to 128-bit via pipe_1to4)
   input  wire [40:0]   ymiso,      // 41 = {ecbor, ecbk[3:0], ecbp[3:0], ecbo[31:0]}  -- AES output           //JLPL - 10_11_25
   output wire          ymiso_fb,   //      {ecbrd}                                    -- this block is ready! //JLPL - 10_11_25
   output wire          parity_error, //JLPL - 11_8_25

   // Global
   input  wire          reset,
   input  wire          clock
   );

// S0  Instruction Word
// S3  AES request Word (only 4-bits)

localparam [4:0]  aS0 =  0, aS1 = 1,  aS2 = 2,  aS3 = 3,
                  aT0 =  4, aT1 = 5,  aT2 = 6,  aT3 = 7,
                  aU0 =  8, aU1 = 9,  aU2 = 10, aU3 = 11,
                  aV0 = 12, aV1 = 13, aV2 = 14, aV3 = 15,
                  aW0 = 16, aW1 = 17, aW2 = 18, aW3 = 19,
                  aX0 = 20, aX1 = 21, aX2 = 22, aX3 = 23,
                  aY0 = 24, aY1 = 25, aY2 = 26, aY3 = 27,
                  aZ0 = 28, aZ1 = 29, aZ2 = 30, aZ3 = 31;

// Locals =======================================================================================================================

// Physical Registers
reg   [127:0]  S,T,U,V,W,X,Y,Z;                   // Registers
// Internal Flip-flops
reg            Parity_fail;
reg            equal;
// Nets
reg   [127:0]  rev;

// Decollate imosi
wire  [ 31:0]  iIO, IOo;
wire  [ 12:0]  aIO;
wire  [  2:0]  cIO;
wire           IOk;
wire  [NUM_REGS-1:0] decode, write, read;
wire           wr, rd, in_range;
//
wire  [254:0]  gfp; //JLPL - 10_21_25
wire  [159:0]  ecb_full;
wire  [127:0]  lhs, rhs, xra, R, gfr, ecb;
wire  [ 31:0]  I;
wire  [ 15:0]  ecb_dest, ecb_parity;
wire  [  7:0]  load;
wire  [  4:0]  aes_rq;
wire           aes_wr;
wire           aes_ir;
wire           ecb_or;
wire           ecb_rd;
wire           same;
wire           execute, waitECB, done;
wire           wait_to_send_to_aes;
wire           wait_to_receive_aes;
wire           startGFM;
wire           requested;

integer  i;

//===============================================================================================================================
// Processor Input/Output  (32-bit)

// S0 = Instruction Register
// S1 = AES request Register
// S2 = 32-bit input to ALU
// S3 = spare

// Decollate input
assign  {cIO[2:0], iIO[31:0], aIO[12:0]} = imosi;

assign
   wr                   =  cIO==3'b110,
   rd                   =  cIO==3'b101,
   in_range             =  aIO[12:LNRG] == BASE[12:LNRG],
   decode[NUM_REGS-1:0] = {{NUM_REGS-1{1'b0}},in_range} << aIO[LNRG-1:0],
   write [NUM_REGS-1:0] = {NUM_REGS{wr}} & decode,
   read  [NUM_REGS-1:0] = {NUM_REGS{rd}} & decode;

A2_mux #(NUM_REGS,32) iIOO (.z( IOo[31:0]),
                            .s(read[NUM_REGS-1:0]),                                 // Addresses
                            .d({ Z[127:96],  Z[95:64],  Z[63:32],  Z[31:0],         //  141f 141e 141d 141c
                                 Y[127:96],  Y[95:64],  Y[63:32],  Y[31:0],         //  141b 141a 1419 1418
                                 X[127:96],  X[95:64],  X[63:32],  X[31:0],         //  1417 1416 1415 1414
                                 W[127:96],  W[95:64],  W[63:32],  W[31:0],         //  1413 1412 1411 1410
                                 V[127:96],  V[95:64],  V[63:32],  V[31:0],         //  140f 140e 140d 140c
                                 U[127:96],  U[95:64],  U[63:32],  U[31:0],         //  140b 140a 1409 1408
                                 T[127:96],  T[95:64],  T[63:32],  T[31:0],         //  1407 1406 1405 1404
                                 S[127:96],  S[95:64],  S[63:32],  S[31:0] }));     //  1403 1402 1401 1400

assign
   IOk   = (|read) | (|write),
   imiso = {IOk, IOo[31:0]};

//===============================================================================================================================
// Registers
//
// Instruction Register (S0)
//         3 3 2 2 2 2 2 2 2 2 2 2 1 1 1 1 1 1 1 1 1 1
//         1 0 9 8 7 6 5 4 3 2 1 0 9 8 7 6 5 4 3 2 1 0 9 8 7 6 5 4 3 3 2 1 0
//        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//      I |  CTRL   | FN  |      dest     |      lhs      |       rhs       |  // One hot selects
//        +---------+-----+---------------+---------------+-----------------+
//
// AES Request Register
//        31                                                         3 2 1 0
//        +---------------------------------------------------------+-+---+-+
//      S |                     previous requests                   |H| S |D|
//        +---------------------------------------------------------+-+---+-+
//                                                                    \  \  \___ Direction  0: From 1: To
//                                                                     \  \
//                                                                      \  \____ Session Number -- 2 bits
//                                                                       \______ Generate an H value 0: H, 1: E
//
//
always @(posedge clock)
   if      (reset)          S[ 31:0 ] <= `tCQ  128'h0;
   else if (write[aS0])     S[ 31:0 ] <= `tCQ  iIO[31:0];            // Instruction Word
   else if (execute)        S[ 31:27] <= `tCQ {  S[30:27],1'b0};     // shift left on execute
   else if (done)           S[ 31:0 ] <= `tCQ   32'h80000000;        // cleared when execution finished
always @(posedge clock)
   if      (write[aS1])     S[ 63:32] <= `tCQ  iIO[31:0];            // AES request Word
   else if (requested)      S[ 63:32] <= `tCQ {  S[59:32],4'h0};     // Save previous requests for debug etc.
always @(posedge clock)
   if      (load[0])        S[127:64] <= `tCQ   R[127:64];
   else if (write[aS2])     S[ 95:64] <= `tCQ  iIO[31:0];            // 32-bit 'constant' into ALU rhs
   else if (write[aS3])     S[127:96] <= `tCQ  iIO[31:0];            // Spare
always @(posedge clock)
   if      (load[1])        T[127:0 ] <= `tCQ   R[127:0];
   else if (write[aT0])     T[ 31:0 ] <= `tCQ  iIO[31:0];
   else if (write[aT1])     T[ 63:32] <= `tCQ  iIO[31:0];
   else if (write[aT2])     T[ 95:64] <= `tCQ  iIO[31:0];
   else if (write[aT3])     T[127:96] <= `tCQ  iIO[31:0];
always @(posedge clock)
   if      (load[2])        U[127:0 ] <= `tCQ   R[127:0];
   else if (write[aU0])     U[ 31:0 ] <= `tCQ  iIO[31:0];
   else if (write[aU1])     U[ 63:32] <= `tCQ  iIO[31:0];
   else if (write[aU2])     U[ 95:64] <= `tCQ  iIO[31:0];
   else if (write[aU3])     U[127:96] <= `tCQ  iIO[31:0];
always @(posedge clock)
   if      (load[3])        V[127:0 ] <= `tCQ   R[127:0];
   else if (write[aV0])     V[ 31:0 ] <= `tCQ  iIO[31:0];
   else if (write[aV1])     V[ 63:32] <= `tCQ  iIO[31:0];
   else if (write[aV2])     V[ 95:64] <= `tCQ  iIO[31:0];
   else if (write[aV3])     V[127:96] <= `tCQ  iIO[31:0];
always @(posedge clock)
   if      (load[4])        W[127:0 ] <= `tCQ   R[127:0];
   else if (write[aW0])     W[ 31:0 ] <= `tCQ  iIO[31:0];
   else if (write[aW1])     W[ 63:32] <= `tCQ  iIO[31:0];
   else if (write[aW2])     W[ 95:64] <= `tCQ  iIO[31:0];
   else if (write[aW3])     W[127:96] <= `tCQ  iIO[31:0];
always @(posedge clock)
   if      (load[5])        X[127:0 ] <= `tCQ   R[127:0];
   else if (write[aX0])     X[ 31:0 ] <= `tCQ  iIO[31:0];
   else if (write[aX1])     X[ 63:32] <= `tCQ  iIO[31:0];
   else if (write[aX2])     X[ 95:64] <= `tCQ  iIO[31:0];
   else if (write[aX3])     X[127:96] <= `tCQ  iIO[31:0];
always @(posedge clock)
   if      (load[6])        Y[127:0 ] <= `tCQ   R[127:0];
   else if (write[aY0])     Y[ 31:0 ] <= `tCQ  iIO[31:0];
   else if (write[aY1])     Y[ 63:32] <= `tCQ  iIO[31:0];
   else if (write[aY2])     Y[ 95:64] <= `tCQ  iIO[31:0];
   else if (write[aY3])     Y[127:96] <= `tCQ  iIO[31:0];
always @(posedge clock)
   if      (load[7])        Z[127:0 ] <= `tCQ   R[127:0];
   else if (write[aZ0])     Z[ 31:0 ] <= `tCQ  iIO[31:0];
   else if (write[aZ1])     Z[ 63:32] <= `tCQ  iIO[31:0];
   else if (write[aZ2])     Z[ 95:64] <= `tCQ  iIO[31:0];
   else if (write[aZ3])     Z[127:96] <= `tCQ  iIO[31:0];

// Instruction ==================================================================================================================

localparam  [2:0]  fnNOP = 0, fnECB = 1, fnREV = 2, fnXOR = 3, fnLHS = 4, fnGFM = 5;

assign
   I[31:0]     =  S[31:0 ],
   load[7:0]   =  done ? I[23:16] : 8'h0,
   execute     = |I[30:27] & ~wait_to_send_to_aes & ~wait_to_receive_aes,
   waitECB     = (I[26:24]==fnECB) & ~ecb_or,
   done        =  (I[30] & ~wait_to_receive_aes) & ~I[29] & ~I[28] & ~I[27];

//===============================================================================================================================
// ALU

A2_mux #(8,128) iLHS (.z(lhs), .s(I[15:8]), .d({Z,Y,X,W,V,U,T,128'h0 }));
A2_mux #(8,128) iRHS (.z(rhs), .s(I[ 7:0]), .d({Z,Y,X,W,V,U,T,96'h0,S[31:0]}));

// Bit reverse
always @* for (i=0;i<128;i=i+1) rev[i] = rhs[127-i];

assign
   xra   =  lhs ~^ rhs, //JLPL - 11_8_25 - Fix Equal Logic to an Exclusive NOR
   same  = &xra,
   R     = {256'h0, gfr, lhs, xra, rev, ecb, 128'h0 } >> (128 * I[26:24]);

// Capture Equal
always @(posedge clock)
   if ((I[26:24]==fnXOR) && I[30])  equal <= same;

//===============================================================================================================================
// Computation - GF Multiply
// Allow 4 cycles of combinatorial delay   I[31:24] = 8'b 00001101  1st cycle
//                                                        00010101  2nd cycle
//                                                        00100101  3rd cycle
//                                                        01000101  4th cycle
//                                                        10000101  Finished
assign
   startGFM =  I[26:24] == fnGFM;

A2_karatsuba_GFmult128 #(.M(128)) i_gfm (
   .z(gfp[254:0]),               // GF Product (255-bit output) //JLPL - 10_21_25
   .x(  X[127:0] ^ Y[127:0]),    // Multiplicand   (X XOR selected input)
   .y(  Z[127:0] )               // Multiplier     (H)
   );

A2_poly_reducer128 i_pr (
   .y(gfr[127:0]),               // Reduced Product
   .x(gfp[254:0])                // Galois Field Product (255-bit input)
   );

//=AES Interface=================================================================================================================

// Request Pipe-------------------
// Wait if aes_ir NOT TRUE and aes_wr TRUE

assign
   wait_to_send_to_aes  =  (I[26:24]==fnECB) & I[29] & ~aes_ir,
   wait_to_receive_aes  =  (I[26:24]==fnECB) & I[30] & ~ecb_or,
   ecb_rd               =  (I[26:24]==fnECB) & I[30],
   aes_wr               =  (I[26:24]==fnECB) & I[29],
   requested            =  (I[26:24]==fnECB) & I[29] &  aes_ir,
   aes_rq               =  {S[3], 1'b1,S[2:0]};                    // H control bit, MSB for GCM, Top 8 key address (only 5 used)

A2_pipe #(5) i_pipe_greq (
   .pdo  (ymosi[4:0] ), //JLPL - 10_11_25 - Revert back to old TileZero Interface Definition
   .por  (ymosi[5]   ), //JLPL - 10_11_25
   .prd  (ymosi_fb   ), //JLPL - 10_11_25
   .pdi  (aes_rq[4:0]),
   .pwr  (aes_wr     ),
   .pir  (aes_ir     ),
   .clock(clock      ),
   .reset(reset      )
   );

// ECB input pipe-----------------
// Wait if ECB_or NOT TRUE nad ECB_rd TRUE

wire amiso_input_ready; //JLPL - 10_16_25

assign ymiso_fb = amiso_input_ready & ymiso[40] & ymiso[39]; //JLPL - 10_16_25 - Change ymiso_fb to combination of pipe_1to4 input ready and amiso write to GCM

A2_pipe_1to4 #(.W(40)) i_gmiso_pipe (
   .ppl_di     ( ymiso[39:0]     ), //JLPL - 10_11_25
   .ppl_wr     ( ymiso[40] && ymiso[39]), //JLPL - 10_11_25 //JLPL - 10_13_25 - Add qualifier for write only address 8 or above
   .ppl_ir     ( amiso_input_ready), //JLPL - 10_11_25 //JLPL - 10_16_25
   .ppl_do     ( ecb_full[159:0] ),             // 4 x 40 = 4 bits dest + 4 bits parity + 32 bits data
   .ppl_rd     ( ecb_rd          ),
   .ppl_or     ( ecb_or          ),
   .ppl_flush  ( reset           ),
   .ppl_endian ( 1'b1            ),             // Big endian
   .clock      ( clock           ),
   .reset      ( reset           )
   );

// Decollate

assign
   ecb_dest  [ 15:0] = {ecb_full[ 39:36 ], ecb_full[ 79:76 ], ecb_full[119:116], ecb_full[159:156]},
   ecb_parity[ 15:0] = {ecb_full[ 35:32 ], ecb_full[ 75:72 ], ecb_full[115:112], ecb_full[155:152]},
   ecb       [127:0] = {ecb_full[ 31:0  ], ecb_full[ 71:40 ], ecb_full[111:80 ], ecb_full[151:120]};

// Parity checking - 16 x byte parity
always @(posedge clock)
   if (reset)
      Parity_fail <= `tCQ  1'b0;
   else if (ecb_or && ecb_rd)
      Parity_fail <= `tCQ (ecb_parity[15:0] != {~^ecb_full[ 31:24 ],~^ecb_full[ 23:16 ],~^ecb_full[ 15:8  ],~^ecb_full[  7:0  ],
                                                ~^ecb_full[ 71:64 ],~^ecb_full[ 63:56 ],~^ecb_full[ 55:48 ],~^ecb_full[ 47:40 ],
                                                ~^ecb_full[111:104],~^ecb_full[103:96 ],~^ecb_full[ 95:88 ],~^ecb_full[ 87:80 ],
                                                ~^ecb_full[151:144],~^ecb_full[143:136],~^ecb_full[135:128],~^ecb_full[127:120]});

// Condition Codes ==============================================================================================================

assign
   cc[1] =   equal,        // lhs == rhs -- Usually for TAG compare
   cc[0] = ~|I[30:27];     // NOT BUSY
   
assign parity_error = Parity_fail; //JLPL - 11_8_25

endmodule
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

`ifdef   verify_GCM2
module t;

parameter   [12:0]   BASE     = 13'h1400;  // Base Address of Co-processor
parameter            NUM_REGS = 32;
parameter            LNRG     = 5;

reg         clock;
reg         gmiso_wr;
reg         gmosi_rd;
reg         reset;
reg  [39:0] gmiso;
reg  [47:0] imosi;

wire        gmiso_ir;
wire        gmosi_or;
wire [ 3:0] gmosi;
wire [ 7:0] cc;
wire [32:0] imiso;

A2_GCM_CoP #(BASE,NUM_REGS,LNRG) mut (
   .imiso    (imiso   [32:0]),
   .cc       (cc      [ 7:0]),
   .gmosi    (gmosi   [ 3:0]),
   .gmosi_or (gmosi_or      ),
   .imosi    (imosi   [47:0]),
   .gmiso    (gmiso   [39:0]),
   .gmosi_rd (gmosi_rd     ),
   .gmiso_wr (gmiso_wr     ),
   .gmiso_ir (gmiso_ir     ),
   .reset    (reset        ),
   .clock    (clock        )
   );

initial begin
   clock    =  1'b0;
   forever  clock = #500 ~clock;
   end

integer  i;

initial begin
   gmiso_wr =  1'b0;
   gmosi_rd =  1'b0;
   gmiso    =  1'b0;
   imosi    = 48'b0;
   reset    =  1'b0;
   repeat (10)    @(posedge clock); #5;
   reset    =  1'b1;
   repeat (10)    @(posedge clock); #5;
   reset    =  1'b0;
   repeat (10)    @(posedge clock); #5;
   // Write to all registers
   for (i=0; i<32; i=i+1) begin
      imosi = {3'b110, {8{i[4:1]}}, (13'h1400 + i[4:0])};  @(posedge clock); #10; if(!imiso[32]) $stop;
      end
   // Readback all registers
   for (i=0; i<32; i=i+1) begin
      imosi = {3'b101, 32'h0, (13'h1400 + i[4:0])};  @(posedge clock); #10; if (imiso[32:0]!={1'b1,{8{i[4:1]}}}) $stop;
      end
   imosi = 48'h00;

   // FUNCTIONS :  fnNOP = 000, fnECB = 001, fnREV = 010, fnXOR = 011, fnLHS = 100, fnGFM = 101;

   // Try a simple move
      imosi = {3'b110, 32'b01000100_11111111_01000000_00000000, 13'h1400};  @(posedge clock); #10; if(!imiso[32]) $stop;
   imosi = 48'h00;   @(posedge clock) #10
   // Try a bit_reverse
      imosi = {3'b110, 32'b01000010_11111111_00000000_10000000, 13'h1400};  @(posedge clock); #10; if(!imiso[32]) $stop;
   imosi = 48'h00;   @(posedge clock) #10
   // Try a XOR
      imosi = {3'b110, 32'b01000011_11111111_01000000_10000000, 13'h1400};  @(posedge clock); #10; if(!imiso[32]) $stop;
   imosi = 48'h00;   @(posedge clock) #10
   // Try a GFM
   imosi = {3'b110, 32'h00000001, 13'h141c};  @(posedge clock); #10; if(!imiso[32]) $stop;
   imosi = {3'b110, 32'h00000010, 13'h141d};  @(posedge clock); #10; if(!imiso[32]) $stop;
   imosi = {3'b110, 32'h00000100, 13'h141e};  @(posedge clock); #10; if(!imiso[32]) $stop;
   imosi = {3'b110, 32'h00001000, 13'h141f};  @(posedge clock); #10; if(!imiso[32]) $stop;
   imosi = {3'b110, 32'h00010000, 13'h141b};  @(posedge clock); #10; if(!imiso[32]) $stop;
   imosi = {3'b110, 32'h00100000, 13'h141a};  @(posedge clock); #10; if(!imiso[32]) $stop;
   imosi = {3'b110, 32'h01000000, 13'h1419};  @(posedge clock); #10; if(!imiso[32]) $stop;
   imosi = {3'b110, 32'h10000000, 13'h1418};  @(posedge clock); #10; if(!imiso[32]) $stop;

   imosi = {3'b110, 32'b01000101_11111111_01000000_10000000, 13'h1400};  @(posedge clock); #10; if(!imiso[32]) $stop;
   imosi = 48'h00;   @(posedge clock) #10



   repeat (100)      @(posedge clock); #5;
   $stop;
   end

endmodule
`endif


