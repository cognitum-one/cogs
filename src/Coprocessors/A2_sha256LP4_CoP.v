/* *******************************************************************************************************************************

   Copyright © 2019 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : CRYPTO Library

   Description          : SHA256 Hash Co-processor
                          Version with 3-stage execution pipeline
                          LP4: Split EFGHI into EFG[0:2] array + individual H,I registers

*********************************************************************************************************************************
   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*********************************************************************************************************************************
 NOTA BENE:  This version uses ABCDEFGH and the Message as little endian vectors.  The FIPS spec implies big endian vectors
             The exception here is the Konstant vector; that is, the index of the most signicant word Ks[2047:2016] is 0. So
             here we have reversed the constant array to keep addressing simple; and loaded it into a ROM
             Hey, what do I know ?!?

             'ok'   is a single cycle pulse and can be used as an interrupt
             Polling can also be done by reading at the GO address.  When the computation is done a read will return
             all 1 (0xffffffff) otherwise it will return zero.
             Note that the GO address must be read to move the state machine back to IDLE.

********************************************************************************************************************************/

`ifdef   STANDALONE
   `define  tCQ   #1
`endif
`ifdef   RELATIVE_FILENAMES                    // DEFINE IN SIMULATOR COMMAND LINE
//   `include "A2_project_settings.vh"
//   `include "A2_defines.vh"
`else
//   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src//include/A2_project_settings.vh"
//   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src//include/A2_defines.vh"
`endif

module   A2_sha256_CoP #(
   parameter   [12:0]   BASE     = 13'h00000,
   parameter            NUM_REGS = 7
   )(
   output wire [32:0]   imiso,   // Slave data out {               IOk, IOo[31:0]}
   input  wire [47:0]   imosi,   // Slave data in  {cIO[2:0] aIO[12:0], iIO[31:0]}
   output wire          ok,      // Finished
   input                reset,
   input                clock
   );

localparam  LG2REGS  =  3;

// Functions ===================================================================================================================
function [63:0] csa42;
input    [31:0] a,b,c,d;
reg      [31:0] e,f,p,t;
begin
          e =  a ^ b;
          f =  c ^ d;
          p =  e ^ f;
          t =  e & c | ~e & b;
csa42[63:32] = (p & (t << 1) | ~p & d) << 1;
csa42[31:00] =  p ^ (t << 1);
end
endfunction

// Rotate Right
`define  ROR(x,d) ((x) >> (d)) | ((x) << (32-(d)))

// Physical Registers ==========================================================================================================

reg       [31:0]  M [0:15];
reg       [31:0]  S [0:7];
reg       [31:0]  ABCD [0:3];
reg       [31:0]  EFG [0:2];     // LP4: Split from EFGHI - 3-word array with one-hot pointer
reg       [31:0]  H, I;          // LP4: Individual registers for H and I
reg       [ 3:0]  Mptr;
reg       [ 2:0]  Sptr;
reg       [ 2:0]  EFGptr;        // LP4: One-hot pointer for static shift
reg       [ 1:0]  ABCDptr;
reg       [ 5:0]  CYCLE;
reg       [ 2:0]  FSM; localparam [2:0]   IDLE = 0, PRIME1 = 1, PRIME2 = 2, COMPUTE = 3, DIGEST = 4, DONE = 5;
reg               RUN;

wire     done     =  FSM   == DONE;
wire     digest   =  FSM   == DIGEST;
wire     compute  =  FSM   == COMPUTE;
wire     cycle1   =  CYCLE == 6'd1;
wire     cycle2   =  CYCLE == 6'd2;
assign   ok       =  digest & (CYCLE==9);

// Parameters From FIPS-180 -----------------------------------------------------------------------------------------------------
localparam [ 255:0] INIT = {
   32'h5be0cd19, 32'h1f83d9ab, 32'h9b05688c, 32'h510e527f, 32'ha54ff53a, 32'h3c6ef372, 32'hbb67ae85, 32'h6a09e667
   };

localparam [2047:0]  Ks  = {
   32'hc67178f2, 32'hbef9a3f7, 32'ha4506ceb, 32'h90befffa, 32'h8cc70208, 32'h84c87814, 32'h78a5636f, 32'h748f82ee,
   32'h682e6ff3, 32'h5b9cca4f, 32'h4ed8aa4a, 32'h391c0cb3, 32'h34b0bcb5, 32'h2748774c, 32'h1e376c08, 32'h19a4c116,
   32'h106aa070, 32'hf40e3585, 32'hd6990624, 32'hd192e819, 32'hc76c51a3, 32'hc24b8b70, 32'ha81a664b, 32'ha2bfe8a1,
   32'h92722c85, 32'h81c2c92e, 32'h766a0abb, 32'h650a7354, 32'h53380d13, 32'h4d2c6dfc, 32'h2e1b2138, 32'h27b70a85,
   32'h14292967, 32'h06ca6351, 32'hd5a79147, 32'hc6e00bf3, 32'hbf597fc7, 32'hb00327c8, 32'ha831c66d, 32'h983e5152,
   32'h76f988da, 32'h5cb0a9dc, 32'h4a7484aa, 32'h2de92c6f, 32'h240ca1cc, 32'h0fc19dc6, 32'hefbe4786, 32'he49b69c1,
   32'hc19bf174, 32'h9bdc06a7, 32'h80deb1fe, 32'h72be5d74, 32'h550c7dc3, 32'h243185be, 32'h12835b01, 32'hd807aa98,
   32'hab1c5ed5, 32'h923f82a4, 32'h59f111f1, 32'h3956c25b, 32'he9b5dba5, 32'hb5c0fbcf, 32'h71374491, 32'h428a2f98
   };

// Parameter ROM ----------------------------------------------------------------------------------------------------------------
wire rROM ;
reg   [31:0]   ROMo;             // ROM output
always @(posedge clock) if (rROM) ROMo <= `tCQ  Ks[32*CYCLE+:32];

// Interface decoding -----------------------------------------------------------------------------------------------------------
`ifdef STANDALONE
   localparam SHA256_MESG = 0,SHA256_HASH = 1,SHA256_STOP = 2,SHA256_STRT_RDY = 3,SHA256_INIT = 4,SHA256_WAKE = 5,SHA256_SLEEP= 6;
`elsif   RELATIVE_FILENAMES
   `include "NEWPORT_IO_addresses.vh"
`else
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src//include/NEWPORT_IO_addresses.vh"
`endif

wire  [31:0]   iIO, IOo;
wire  [12:0]   aIO;
wire  [ 2:0]   cIO;
// Decollate imosi
assign   {cIO, iIO, aIO} = imosi;

wire                  in_range =  aIO[12:LG2REGS]  == BASE[12:LG2REGS];
wire  [NUM_REGS-1:0]    decode =  in_range << aIO[LG2REGS-1:0];
wire  [NUM_REGS-1:0]     write =  {NUM_REGS{(cIO==3'b110)}} & decode;
wire  [NUM_REGS-1:0]      read =  {NUM_REGS{(cIO==3'b101)}} & decode;

// Writes
wire  ld_M =  write[SHA256_MESG],      // Push Message data W0 first  (FIPS180 notation)
      ld_H =  write[SHA256_HASH],      // Push Hash    data H7 first  (FIPS180 notation)
      stop =  write[SHA256_STOP],      // Stop the co-processor and reset
      go   =  write[SHA256_STRT_RDY],  // Start processing
      init =  write[SHA256_INIT],      // Load standard Initial Hash value
      wake =  write[SHA256_WAKE],      // Wake-up the CoProcessor
     sleep =  write[SHA256_SLEEP];     // Put the CoProcessor to sleep
// Reads
wire  rd_M =   read[SHA256_MESG],      // Pop  Message data W0 first  (FIPS180 notation)
      rd_H =   read[SHA256_HASH],      // Pop  Digest  data H7 first  (FIPS180 notation)
      fini =   read[SHA256_STRT_RDY],  // Read status:  if finished TRUE else FALSE
      up   =   read[SHA256_WAKE];      // Read status:  If awake    TRUE else FALSE

wire  IOk  =  |write[NUM_REGS-1:0] |  |read[NUM_REGS-1:0];

assign
      rROM = (FSM==IDLE) & go | (FSM==PRIME1) | (FSM==PRIME2) | (FSM==COMPUTE) & (CYCLE>2);

always @(posedge clock)
   if (reset || sleep) RUN <= `tCQ 1'b0;
   else if  (  wake  ) RUN <= `tCQ 1'b1;

// Message processing -----------------------------------------------------------------------------------------------------------
// Barrel Shifter for Message array - extract w14, w9, w1, w0 in correct logical order
wire  [31:0]   ms15,ms14,ms13,ms12,ms11,ms10,ms9,ms8,ms7,ms6,ms5,ms4,ms3,ms2,ms1,ms0,
               mt15,mt14,mt13,mt12,mt11,mt10,mt9,mt8,mt7,mt6,mt5,mt4,mt3,mt2,mt1,mt0,
               mu15,mu14,mu13,mu12,mu11,mu10,mu9,mu8,mu7,mu6,mu5,mu4,mu3,mu2,mu1,mu0,
               mv15,mv14,mv13,mv12,mv11,mv10,mv9,mv8,mv7,mv6,mv5,mv4,mv3,mv2,mv1,mv0,
               mw15,mw14,mw13,mw12,mw11,mw10,mw9,mw8,mw7,mw6,mw5,mw4,mw3,mw2,mw1,mw0;

wire  [31:0]   w14, w9, w1, w0;

assign
   {ms15,ms14,ms13,ms12,ms11,ms10,ms9,ms8,ms7,ms6,ms5,ms4,ms3,ms2,ms1,ms0} =
      {M[15],M[14],M[13],M[12],M[11],M[10],M[9],M[8],M[7],M[6],M[5],M[4],M[3],M[2],M[1],M[0]},

   {mt15,mt14,mt13,mt12,mt11,mt10,mt9,mt8,mt7,mt6,mt5,mt4,mt3,mt2,mt1,mt0} =
      Mptr[3]  ?  {ms7,ms6,ms5,ms4,ms3,ms2,ms1,ms0,ms15,ms14,ms13,ms12,ms11,ms10,ms9,ms8}
               :  {ms15,ms14,ms13,ms12,ms11,ms10,ms9,ms8,ms7,ms6,ms5,ms4,ms3,ms2,ms1,ms0},

   {mu15,mu14,mu13,mu12,mu11,mu10,mu9,mu8,mu7,mu6,mu5,mu4,mu3,mu2,mu1,mu0} =
      Mptr[2]  ?  {mt3,mt2,mt1,mt0,mt15,mt14,mt13,mt12,mt11,mt10,mt9,mt8,mt7,mt6,mt5,mt4}
               :  {mt15,mt14,mt13,mt12,mt11,mt10,mt9,mt8,mt7,mt6,mt5,mt4,mt3,mt2,mt1,mt0},

   {mv15,mv14,mv13,mv12,mv11,mv10,mv9,mv8,mv7,mv6,mv5,mv4,mv3,mv2,mv1,mv0} =
      Mptr[1]  ?  {mu1,mu0,mu15,mu14,mu13,mu12,mu11,mu10,mu9,mu8,mu7,mu6,mu5,mu4,mu3,mu2}
               :  {mu15,mu14,mu13,mu12,mu11,mu10,mu9,mu8,mu7,mu6,mu5,mu4,mu3,mu2,mu1,mu0},

   {mw15,mw14,mw13,mw12,mw11,mw10,mw9,mw8,mw7,mw6,mw5,mw4,mw3,mw2,mw1,mw0} =
      Mptr[0]  ?  {mv0,mv15,mv14,mv13,mv12,mv11,mv10,mv9,mv8,mv7,mv6,mv5,mv4,mv3,mv2,mv1}
               :  {mv15,mv14,mv13,mv12,mv11,mv10,mv9,mv8,mv7,mv6,mv5,mv4,mv3,mv2,mv1,mv0};
// Chunk
assign
   w14   =  mw14,
   w9    =  mw9,
   w1    =  mw1,
   w0    =  mw0;

// Message
wire  [31:0]   sig0  = (`ROR(w1,  7)) ^ (`ROR(w1, 18)) ^ (w1  >> 3 ),
               sig1  = (`ROR(w14,17)) ^ (`ROR(w14,19)) ^ (w14 >> 10);
wire  [63:0]   p0    =  csa42(w0,w9,sig0,sig1);
wire  [31:0]   nW    =  p0[63:32] + p0[31:0];

// Hash S register -----------------------------------------------------------------------------------------------------------
// Barrel Shifter for S array - extract all 8 positions
wire  [31:0]   ss7,ss6,ss5,ss4,ss3,ss2,ss1,ss0,
               st7,st6,st5,st4,st3,st2,st1,st0,
               su7,su6,su5,su4,su3,su2,su1,su0;
wire  [31:0]   s7,s6,s5,s4,s3,s2,s1,s0;

assign
   {ss7,ss6,ss5,ss4,ss3,ss2,ss1,ss0} =            {S[7],S[6],S[5],S[4],S[3],S[2],S[1],S[0]},
   {st7,st6,st5,st4,st3,st2,st1,st0} = Sptr[2]  ? {ss3,ss2,ss1,ss0,ss7,ss6,ss5,ss4} : {ss7,ss6,ss5,ss4,ss3,ss2,ss1,ss0},
   {su7,su6,su5,su4,su3,su2,su1,su0} = Sptr[1]  ? {st1,st0,st7,st6,st5,st4,st3,st2} : {st7,st6,st5,st4,st3,st2,st1,st0},
   {s7,s6,s5,s4,s3,s2,s1,s0}         = Sptr[0]  ? {su0,su7,su6,su5,su4,su3,su2,su1} : {su7,su6,su5,su4,su3,su2,su1,su0};

// ABCD register ----------------------------------------------------------------------------------------------------------------
// Barrel Shifter for ABCD array - ABCDptr points to NEXT write location (oldest)
wire  [31:0]   A = ABCD[(ABCDptr + 2'd3) & 2'h3],  // Newest (ptr-1)
               B = ABCD[(ABCDptr + 2'd2) & 2'h3],
               C = ABCD[(ABCDptr + 2'd1) & 2'h3],
               D = ABCD[(ABCDptr + 2'd0) & 2'h3];  // Oldest (ptr)

// EFG register (LP4) -----------------------------------------------------------------------------------------------------------
// Static shift with one-hot pointer - only ONE register clocks per cycle!
// Pointer indicates oldest position (G), which will be overwritten with new E
// After write, pointer rotates, making that position the newest (E)
wire  [31:0]   G = EFGptr[0] ? EFG[0] : (EFGptr[1] ? EFG[1] : EFG[2]),     // Oldest at ptr
               F = EFGptr[0] ? EFG[1] : (EFGptr[1] ? EFG[2] : EFG[0]),     // (ptr+1)%3
               E = EFGptr[0] ? EFG[2] : (EFGptr[1] ? EFG[0] : EFG[1]);     // Newest at (ptr-1+3)%3
// H and I are individual registers

// Hash stage 0 -----------------------------------------------------------------------------------------------------------------
wire  [63:0]   tmp0  =  csa42(ROMo, w0, G, B);  // G, B are pipelined H and D

// Hash stage 1 -----------------------------------------------------------------------------------------------------------------
wire  [31:0]   S1    =  digest ? 32'h0 : ((`ROR(E,6)) ^ (`ROR(E,11)) ^ (`ROR(E,25))),
               ch    =  digest ? 32'h0 :  E & F |~E & G;
wire  [63:0]   p1    =  csa42(H,I,ch,S1);
wire  [31:0]   tmp1  =  p1[63:32] + p1[31:0];

// Hash stage 2 -----------------------------------------------------------------------------------------------------------------
wire  [31:0]   S0    = ((`ROR(A,2)) ^ (`ROR(A,13)) ^ (`ROR(A,22))),
               mj    =  A & B | B & C | C & A;
wire  [63:0]   p2    =  csa42(mj,S0,E,~D);
wire  [31:0]   tmp2  =  p2[63:32] + p2[31:0] + 1'b1;

// Combinatorial logic to capture next values for EFG, H, I
wire  [31:0]   next_I, next_H, next_G, next_F, next_E;

assign
   {next_I,next_H,next_G,next_F,next_E} = compute && cycle1 ?  { 32'h0,    G, F, E, tmp1 }
                                        : compute && cycle2 ?  { s7,       H, G, F, E    }
                                        : compute           ?  { tmp0[63:00], F, E, tmp1 }
                                        : digest            ?  { s7,       G, F, E, D    }
                                        :                      { I,        H, G, F, E    };

always @(posedge clock)
   if (reset || stop) begin
      FSM      <= `tCQ IDLE;
      CYCLE    <= `tCQ 6'd0;
      Mptr     <= `tCQ 4'd0;
      Sptr     <= `tCQ 3'd0;
      ABCDptr  <= `tCQ 2'd0;
      EFGptr   <= `tCQ 3'b001;  // One-hot: start at position 0
      H        <= `tCQ 32'h0;
      I        <= `tCQ 32'h0;
      end
   else if (RUN) (* parallel_case *) case(FSM)                       // **** RK: Make sure this is a clock gate!
      IDLE : if (rd_M)
               Mptr     <= `tCQ Mptr + 1'b1;
         else if (ld_M) begin
               M[Mptr]  <= `tCQ iIO[31:0];
               Mptr     <= `tCQ Mptr + 1'b1;
               end
         else if (rd_H)
               Sptr     <= `tCQ Sptr - 1'b1;
         else if (ld_H) begin
               S[7-Sptr]<= `tCQ iIO[31:0];
               Sptr     <= `tCQ Sptr + 1'b1;
               end
         else if (init) begin
              {S[7],S[6],S[5],S[4],S[3],S[2],S[1],S[0]} <= INIT[255:0];
               Sptr     <= `tCQ 3'd0;
               end
         else if (go  ) begin
               ABCD[0]  <= `tCQ s2;                                        // Broadside load D=s2
               ABCD[1]  <= `tCQ s1;                                        // C=s1
               ABCD[2]  <= `tCQ s3;                                        // B=s3
               ABCD[3]  <= `tCQ s0;                                        // A=s0
               ABCDptr  <= `tCQ 2'd0;                                      // ptr=0, will shift in COMPUTE
               // LP4: Split EFGHI into EFG[0:2] + H,I (static shift with one-hot ptr)
               I        <= `tCQ 32'h0;                                     // I=0
               H        <= `tCQ s6;                                        // H=s6 (note: {I,G,H,F,E})
               // Initialize with ptr=3'b001: G at [0], F at [1], E at [2]
               EFG[0]   <= `tCQ s7;                                        // G=s7 (oldest)
               EFG[1]   <= `tCQ s5;                                        // F=s5 (middle)
               EFG[2]   <= `tCQ s4;                                        // E=s4 (newest)
               EFGptr   <= `tCQ 3'b001;                                    // One-hot: ptr at position 0
               FSM      <= `tCQ PRIME1;
               CYCLE    <= `tCQ CYCLE + 1'b1;
               end
      PRIME1 : begin
               M[Mptr]  <= `tCQ nW;
               Mptr     <= `tCQ Mptr + 1'b1;
               ABCD[0]  <= `tCQ C;                                         // Rotate {D,C,B}: D<=C
               ABCD[1]  <= `tCQ B;                                         // C<=B
               ABCD[2]  <= `tCQ D;                                         // B<=D
               // LP4: {I,H,G,F,E} <= {tmp0, H, F, E}  (G←H, F and E hold)
               I        <= `tCQ tmp0[63:32];
               H        <= `tCQ tmp0[31:00];
               EFG[0]   <= `tCQ H;                                         // G ← old H (essential!)
               // Don't rotate pointer - F and E hold, so keep same logical positions
               CYCLE    <= `tCQ  CYCLE + 1'b1;
               FSM      <= `tCQ  PRIME2;
               end
      PRIME2 : begin
               M[Mptr]  <= `tCQ nW;
               Mptr     <= `tCQ Mptr + 1'b1;
               ABCD[0]  <= `tCQ C;                                         // Rotate {D,C,B}: D<=C
               ABCD[1]  <= `tCQ B;                                         // C<=B
               ABCD[2]  <= `tCQ D;                                         // B<=D
               // LP4: {I,H,G,F,E} <= {tmp0, F,E,tmp1} - broadside load before static shift starts
               I        <= `tCQ tmp0[63:32];
               H        <= `tCQ tmp0[31:00];
               EFG[0]   <= `tCQ F;                                         // G <= F
               EFG[1]   <= `tCQ E;                                         // F <= E
               EFG[2]   <= `tCQ tmp1;                                      // E <= tmp1
               // Pointer stays at 3'b001 - will write to position 0 first in COMPUTE
               CYCLE    <= `tCQ  CYCLE + 1'b1;
               FSM      <= `tCQ  COMPUTE;
               end
      COMPUTE: begin
               M[Mptr]  <= `tCQ nW;
               Mptr     <= `tCQ Mptr + 1'b1;
               // ABCD - static shift (simple shift pattern: D←C←B←A←tmp2)
               ABCD[ABCDptr]  <= `tCQ tmp2;
               ABCDptr        <= `tCQ ABCDptr + 1'b1;
               // LP4: Static shift for EFG - only write next_E to ONE position!
               I        <= `tCQ next_I;
               H        <= `tCQ next_H;
               // Static shift: write new E to oldest position, rotate pointer
               // EXCEPT on CYCLE 2 where we HOLD (don't write, don't rotate)
               if (CYCLE != 02) begin
                  if (EFGptr[0]) EFG[0] <= `tCQ next_E;  // Only ONE of these fires per cycle
                  if (EFGptr[1]) EFG[1] <= `tCQ next_E;
                  if (EFGptr[2]) EFG[2] <= `tCQ next_E;
                  EFGptr <= `tCQ {EFGptr[1:0], EFGptr[2]};  // Rotate one-hot pointer
                  end
               // Special handling for CYCLE 2: transition to DIGEST
               if (CYCLE == 02) begin
                  Sptr   <= `tCQ  Sptr  - 1'b1;
                  FSM    <= `tCQ  DIGEST;
                  end
               CYCLE     <= `tCQ  CYCLE + 1'b1;
               end
      DIGEST:  begin
               ABCD[ABCDptr]  <= `tCQ H;                                         // Static shift <- H
               ABCDptr   <= `tCQ ABCDptr + 1'b1;
               // LP4: Static shift for EFG in DIGEST
               I         <= `tCQ next_I;
               H         <= `tCQ next_H;
               // Static shift: write new E (which is D) to oldest position, rotate pointer
               if (EFGptr[0]) EFG[0] <= `tCQ next_E;  // Only ONE of these fires per cycle
               if (EFGptr[1]) EFG[1] <= `tCQ next_E;
               if (EFGptr[2]) EFG[2] <= `tCQ next_E;
               EFGptr    <= `tCQ {EFGptr[1:0], EFGptr[2]};  // Rotate one-hot pointer
               S[Sptr]   <= `tCQ  tmp1;                                     // Write tmp1
               Sptr      <= `tCQ  Sptr - 1'b1;                              // Decrement pointer
               if (CYCLE == 10) FSM <= `tCQ  DONE;
               CYCLE     <= `tCQ  CYCLE + 1'b1;
               end
      DONE  :  if (fini) begin
                  FSM    <= `tCQ  IDLE;
                  CYCLE  <= `tCQ  6'h0;
                  Sptr   <= `tCQ  3'd0;
                  ABCDptr<= `tCQ  2'd0;
                  end
      endcase

// Outputs ----------------------------------------------------------------------------------------------------------------------

assign
   IOo   =  rd_H           ?  s7
         :  rd_M           ?  w0
         :  fini && done
         ||   up && RUN    ?  32'hffffffff : 32'h00000000;

// Collate
assign
   imiso = {IOk, IOo};

endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

`ifdef   verify_sha256           // DEFINE IN SIMULATOR Command Line

module   t;

parameter   PERIOD      =  10;


integer        count, i;
reg   [511:0]  mesg;
reg   [255:0]  hash;
reg   [255:0]  xpct;

wire   [32:0]  imiso;
wire           hash_ok;

reg    [47:0]  imosi;

reg            reset;
reg            clock;

A2_sha256_CoP mut (
   .imiso   (imiso),
   .imosi   (imosi),
   .ok      (hash_ok),
   .reset   (reset),
   .clock   (clock)
   );

initial begin
   clock =  1'b0;
   forever begin
      #(PERIOD/2) clock = 1'b1;
      #(PERIOD/2) clock = 1'b0;
      end
   end

wire [31:0] H7 = mut.H + mut.I;
reg  [31:0] expect;

always @(posedge clock) count = reset ? 0 : count +1;

`ifdef STANDALONE
   `define index_SHA256_MESG     13'd0
   `define index_SHA256_HASH     13'd1
   `define index_SHA256_STOP     13'd2
   `define index_SHA256_STRT     13'd3
   `define index_SHA256_INIT     13'd4
   `define index_SHA256_WAKE     13'd5
   `define index_SHA256_SLEEP    13'd6
`endif

initial begin
   $display(" Testing SHA256 with separate Shift registers \n");
   reset =  0;
   #10;
   reset =  1;
   #50;
   reset =  0;
   #10;
   // Wake
   imosi =  {3'b110,32'h1,`index_SHA256_WAKE};   @(posedge clock); #2;
   imosi =  {3'b000,32'h0,13'h0};

   xpct  =   256'h0b28938404c4bd4ac42dd095b0921451593a803eaf6ddfcff9e3fe4dfaae9656;
   hash  =   256'h5be0cd191f83d9ab9b05688c510e527fa54ff53a3c6ef372bb67ae856a09e667;
   mesg  =  {256'hc791d4646240fc2a2d1b80900020a24dc501ef1599fc48ed6cbac920af755756,
             256'h18e7b1e8eaf0b62a90d1942ea64d250357e9a09c063a4782_7c57b44e_01000000};

   @(posedge clock); #2
   for (i=0; i<16; i=i+1) begin
      imosi =  {3'b110, mesg[32*i +: 32],     `index_SHA256_MESG};   @(posedge clock); #2;
      imosi =  {3'b000, 32'h0,                `index_SHA256_MESG};   @(posedge clock); #2;
      end

   @(posedge clock); #2
   for (i=0; i<16; i=i+1) begin
      expect = mesg[32*i+:32];
      imosi =  {3'b101, mesg[32*i +: 32],     `index_SHA256_MESG};   @(posedge clock);
      if (expect!==imiso[31:0]) $display(" Error : Message read back  -- Expected %h  Found %h", expect,imiso[31:0]);
      imosi =  {3'b000, 32'h0,                `index_SHA256_MESG};   @(posedge clock); #2;
      end

   for (i=0; i<8; i=i+1) begin
      imosi =  {3'b110, hash[32*(7-i) +: 32], `index_SHA256_HASH};   @(posedge clock); #2;
      imosi =  {3'b000, 32'h0,                `index_SHA256_HASH};   @(posedge clock); #2;
      imosi =  {3'b000, 32'h0,                `index_SHA256_HASH};   @(posedge clock); #2;
      imosi =  {3'b000, 32'h0,                `index_SHA256_HASH};   @(posedge clock); #2;
      end

      imosi =  {3'b110, 32'h0,                `index_SHA256_STRT};   @(posedge clock); #2;
      imosi =  {3'b000, 32'h0,                13'h0        };        @(posedge clock); #2;

      imosi =  {3'b101, 32'h0,                `index_SHA256_STRT};   @(posedge clock); #2;
   while (imiso=={1'b1,32'h0}) begin
      imosi =  {3'b000, 32'h0,                13'h0};   @(posedge clock); #2;
      imosi =  {3'b000, 32'h0,                13'h0};   @(posedge clock); #2;
      imosi =  {3'b000, 32'h0,                13'h0};   @(posedge clock); #2;
      imosi =  {3'b000, 32'h0,                13'h0};   @(posedge clock); #2;
      imosi =  {3'b000, 32'h0,                13'h0};   @(posedge clock); #2;
      imosi =  {3'b101, 32'h0,                13'h3};   @(posedge clock); #2;
      end
   @(posedge clock); #2;

   for (i=0; i<8; i=i+1) begin
      imosi =  {3'b101, 32'h0, 13'h1}; @(posedge clock) hash[32*(7-i)  +: 32] <= #2 imiso[31:0];
      end
   imosi = {3'b000, 32'h0, 13'h0};

   @(posedge clock); #2;
   if (xpct  == hash) $display("\n Test 1 Success:  Found %h",hash);
   else               $display("\n Test 1 Failure:  Found %h",hash);
   #100;

   hash  =   256'h0b28938404c4bd4ac42dd095b0921451593a803eaf6ddfcff9e3fe4dfaae9656;
   mesg  =  {256'h0000028000000000000000000000000000000000000000000000000000000000,
             256'h000000000000000000000000800000000000318f7e71441b141fe951b2b0c7df};
   xpct  =   256'h78e086ecb7819d39318a7fcc59eb59bc9df15205503ecd66d4d43bd32a02a240;

   @(posedge clock); #2
   for (i=0; i<16; i=i+1) begin
      imosi =  {3'b110, mesg[32*i  +: 32], 13'h0};  @(posedge clock); #2;
      end
   for (i=0; i<8; i=i+1) begin
      imosi =  {3'b110,hash[32*(7-i)  +: 32],13'h1};  @(posedge clock); #2;
      end

   imosi =  {3'b110, 32'h0, 13'h3            };  @(posedge clock); #2
   while (imiso=={1'b1,32'h0}) begin
      imosi =  {3'b101, 32'h0, 13'h3};           @(posedge clock); #2;
      end
   @(posedge clock); #2;

   for (i=0; i<8; i=i+1) begin
      imosi =  {3'b101, 32'h0, 13'h1}; @(posedge clock) hash[32*(7-i)  +: 32] <= #2 imiso[31:0];
      end
   imosi = {3'b000, 32'h0, 13'h0};

   @(posedge clock); #2;
   if (xpct  == hash) $display("\n Test 2 Success:  Found %h",hash);
   else               $display("\n Test 2 Failure:  Found %h",hash);
   #100;

// -----------------------------------------------------------------------------------------------------------------------------

   $display("\n FIPS-180 Standard Test Case :");

   imosi = {3'b000,13'h0,32'h0};
   xpct  =  256'hf20015adb410ff6196177a9cb00361a35dae2223414140de8f01cfeaba7816bf;
   hash  =  256'h5be0cd191f83d9ab9b05688c510e527fa54ff53a3c6ef372bb67ae856a09e667;
   mesg  = {256'h6162638000000000000000000000000000000000000000000000000000000000,
            256'h0000000000000000000000000000000000000000000000000000000000000018};

//   xpct  =   256'h0b28938404c4bd4ac42dd095b0921451593a803eaf6ddfcff9e3fe4dfaae9656;
//   hash  =   256'h5be0cd191f83d9ab9b05688c510e527fa54ff53a3c6ef372bb67ae856a09e667;
//   mesg  =  {256'hc791d4646240fc2a2d1b80900020a24dc501ef1599fc48ed6cbac920af755756,
//             256'h18e7b1e8eaf0b62a90d1942ea64d250357e9a09c063a47827c57b44e01000000};

   $display("\n   The initial Hash value is               \n\n              %h %h %h %h %h %h %h %h",
      hash[32*0+:32],
      hash[32*1+:32],
      hash[32*2+:32],
      hash[32*3+:32],
      hash[32*4+:32],
      hash[32*5+:32],
      hash[32*6+:32],
      hash[32*7+:32]
      );

   $display("\n   The initial Message is                  \n\n              %h %h %h %h %h %h %h %h",
      mesg[32*(15-0)+:32],
      mesg[32*(15-1)+:32],
      mesg[32*(15-2)+:32],
      mesg[32*(15-3)+:32],
      mesg[32*(15-4)+:32],
      mesg[32*(15-5)+:32],
      mesg[32*(15-6)+:32],
      mesg[32*(15-7)+:32]
      );
   $display("              %h %h %h %h %h %h %h %h",
      mesg[32*(15-8) +:32],
      mesg[32*(15-9) +:32],
      mesg[32*(15-10)+:32],
      mesg[32*(15-11)+:32],
      mesg[32*(15-12)+:32],
      mesg[32*(15-13)+:32],
      mesg[32*(15-14)+:32],
      mesg[32*(15-15)+:32]
      );

   @(posedge clock); #2
   for (i=0; i<16; i=i+1) begin
      imosi =  {3'b110,mesg[32*(15-i) +: 32],13'h0};  @(posedge clock); #2;
      end
   imosi =     {3'b110, 32'h0, 13'h4};   @(posedge clock); #2    // init
   imosi =     {3'b110, 32'h0, 13'h3};   @(posedge clock); #2    // go

   while (imiso=={1'b1, 32'h0}) begin
      imosi =  {3'b101, 32'h0, 13'h3};   @(posedge clock); #2;
      end
   @(posedge clock); #2;

   for (i=0; i<8; i=i+1) begin
      imosi =  {3'b101, 32'h0, 13'h1}; @(posedge clock) hash[32*(7-i)  +: 32] <= #2 imiso[31:0];
      end

   @(posedge clock); #2;
   if (xpct  == hash) $display("\n        Success:  Found %h",hash);
   else begin         $display("\n        Failure:  Found %h",hash);
                      $display(  "               Expected %h",xpct); end
   $display("\n   The resulting 256-bit message digest is \n\n              %h %h %h %h %h %h %h %h",
      hash[32*0+:32],
      hash[32*1+:32],
      hash[32*2+:32],
      hash[32*3+:32],
      hash[32*4+:32],
      hash[32*5+:32],
      hash[32*6+:32],
      hash[32*7+:32]
      );
   #100;


   $display("\n DONE  \n");
   $finish;
   end

endmodule
`endif
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

