// *******************************************************************************************************************************
//
//    Copyright © 1996..2024 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to Authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//    Project Name         : A2 Library Component
//    Description          : Integer Multipliers
//
//                           Baugh Wooley signed/unsigned radix 2 sequential multiply
//                           CSA accumulation for high speed
//
//
//******************************************************************************************************************************
//    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//******************************************************************************************************************************
//    Notes for Use and Synthesis
//
//
//******************************************************************************************************************************

`ifdef  synthesis //JLPL
   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/Top/COGNITUM_project_settings.v"
`else
  `include "A2_project_settings.vh"
`endif

//`ifndef  tCQ
//`define  tCQ   #1
//`endif

module A2_BWmultiply_BSW  (                  //  BSW:  Byte\Short\Word
   output wire [31:0]   LPo,                 //  Lower Product: Integer result of multiply
   output wire [31:0]   UPPo,                //  Upper Partial Product
   output wire [31:0]   UPCo,                //  Upper Partial Carry
   output wire          done,                //  Output is valid
   input  wire [31:0]   iMC,                 //  Multiplicand Operand to be multiplied
   input  wire [31:0]   iMP,                 //  Multiplier   operand to multiply by
   input  wire [ 7:0]   iBS,                 //  Byte shift input
   input  wire          ldBW,                //  load BW multiplier
   input  wire [ 4:0]   fn,                  //  Shift bytes,signed\unsigned, 32-bit, 16-bit, 8-bit -- one-hot
   input  wire [23:0]   setup,               //  Timing setup duration for clock ratio between fast and slow
   input  wire          fast_clock,
   input  wire          slow_clock,
   input  wire          reset
   );

localparam  if8 = 0, if16 = 1, if32 = 2, sign = 3, shift = 4;

wire  [3:0] lo,li,uo,ui,mb,ms;               //  Expansion signals

// Control ----------------------------------------------------------------------------------------------------------------------

// Fast and slow clocks are an integer ratio apart. So ignoring skew there is a fast clock rising edge approximately one fast
// clock cycle time from the slow clock edge.  The fast clock rising edge should be later than the slow clock edge to
// avoid metastability issues.
// Given this, a fast decode of ldBW can be clocked by the slow clock and shortly thereafter by the fast clock.  This sets up the
// fast clock cycling.
//  8 bit multiply requires  8 fast cycles
// 16 bit multiply requires 16 fast cycles
// 32 bit multiply requires 32 fast cycles
// 16 byte  shift  requires 16 fast cycles
//  4 byte  shift  requires  4 fast cycles -- for test only
// If the fast clock is nominally 12 GHz and the slow clock 2.4 GHz there is a 5:1 ratio. So:
//  8 bit multiply requires  2 slow cycles with 2 fast clock cycles to spare
// 16 bit multiply requires  4 slow cycles with 4 fast clock cycles to spare
// 32 bit multiply requires  7 slow cycles with 3 fast clock cycles to spare
// 16 byte shift   requires  4 slow cycles with 4 fast clock cycles to spare
//  4 byte  shift  requires  1 slow cycle  with no spare
// So one fast cycle to start and one to finish is good for all cases

// Configuration                            2 2 2 2 1 1 1 1 1 1 1 1 1 1
//                                          3 2 1 0 9 8 7 6 5 4 3 2 1 0 9 8 7 6 5 4 3 2 1 0
//                         +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//                         |///////////////| delay2| delay | shift | if32  | if16  |  if8  |
//                         +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//                Default                      3       2       3       5       3       2     => 32'h0032_3532

reg   [4:0] count;
reg   [7:0] fast_sync;
reg   [3:0] slow_count;

wire        slow_count_zero   =  slow_count == 4'h0;
wire        idle              =  slow_count == 4'hf;
wire        last              =       count == 5'h00;
wire        load              =  fast_sync[setup[18:16]] & ~fast_sync[setup[22:20]];
wire        signd             =  fn[sign];
wire        sBS               =  fn[shift];

always @(posedge slow_clock)
   if      (reset) slow_count <= `tCQ  4'hf;
   else if (ldBW)  slow_count <= `tCQ  fn[shift]   ?  setup[15:12]    // defaults 4'h3
                               :       fn[if16]    ?  setup[ 7:4 ]    //          4'h3
                               :       fn[if32]    ?  setup[11:8 ]    //          4'h5
                               :                      setup[ 3:0 ];   //          4'h2

   else if (!idle) slow_count <= `tCQ  slow_count_zero ? 4'hf : slow_count - 1;

always @(posedge fast_clock)  fast_sync <= `tCQ {fast_sync[6:0], ldBW};

always @(posedge fast_clock)
   if (reset)     count <= `tCQ 5'h00;
   else if (load) count <= `tCQ fn[shift] || fn[if16] ?  5'h0f
                        :       fn[if32]              ?  5'h1f
                        :                                5'h07;
   else if (last) count <= `tCQ 5'h00;
   else           count <= `tCQ count - 1'b1;

assign      done  = slow_count_zero;

// Slices -----------------------------------------------------------------------------------------------------------------------

A2_BW_multiply  slice0 (
   .LPo  ( LPo[7:0]),                        //  Lower Product: Integer result of multiply
   .UPPo (UPPo[7:0]),                        //  Upper Partial Product
   .UPCo (UPCo[7:0]),                        //  Upper Partial Carry
   .iMC  ( iMC[7:0]),                        //  Multiplicand Operand to be multiplied
   .iMP  ( iMP[7:0]),                        //  Multiplier   Operand to multiply by
   .iBS  ( LPo[15:8]),                       //  Byte shift in
   .ldBW (load),                             //  load BW multiplier
   .signd(signd),                            //  signed\unsigned
   .shft (sBS),                              //  Byte shift
   .last (last),
   // Expansion outputs
   .lo   (lo[0]),                            //  multiplier serial out
   .uo   (uo[0]),                            //  upper sum serial out
   // Expansion inputs
   .li   (li[0]),                            //  multiplier serial in
   .ui   (ui[0]),                            //  upper sum serial in
   .mb   (mb[0]),                            //  multiply bit to partial product generation
   .ms   (ms[0]),                            //  most significant bits for each size of multiply
   // Global
   .clock(fast_clock)
   );

assign
   li[0] =  fn[if8]  ?  uo[0] : lo[1],
   ui[0] =  fn[if8]  ?  1'b0  : uo[1],
   mb[0] =  lo[0],
   ms[0] =  fn[if8];

A2_BW_multiply  slice1 (
   .LPo  ( LPo[15:8]),                       //  Lower Product: Integer result of multiply
   .UPPo (UPPo[15:8]),                       //  Upper Partial Product
   .UPCo (UPCo[15:8]),                       //  Upper Partial Carry
   .iMC  ( iMC[15:8]),                       //  Multiplicand Operand to be multiplied
   .iMP  ( iMP[15:8]),                       //  Multiplier   operand to multiply by
   .iBS  ( LPo[23:16]),                      //  Byte shift in
   .ldBW (load),                             //  load BW multiplier
   .signd(signd),                            //  signed\unsigned
   .shft (sBS),                              //  Byte shift
   .last (last),                             //  last cycle
   .lo   (lo[1]),                            //  multiplier serial out
   .uo   (uo[1]),                            //  upper sum serial out
   .li   (li[1]),                            //  multiplier serial in
   .ui   (ui[1]),                            //  upper sum serial in
   .mb   (mb[1]),                            //  multiply bit to partial product generation
   .ms   (ms[1]),                            //  most significant bits for each size of multiply
   .clock(fast_clock)
   );

assign
   li[1] =  fn[if8]  ? uo[1]
         :  fn[if16] ? uo[0] : lo[2],
   ui[1] =  fn[if32] ? uo[2] :  1'b0,
   mb[1] =  fn[if8]  ? lo[1] : lo[0],
   ms[1] =  fn[if8]  | fn[if16];

A2_BW_multiply  slice2 (
   .LPo  ( LPo[23:16]),                      //  Lower Product: Integer result of multiply
   .UPPo (UPPo[23:16]),                      //  Upper Partial Product
   .UPCo (UPCo[23:16]),                      //  Upper Partial Carry
   .iMC  ( iMC[23:16]),                      //  Multiplicand Operand to be multiplied
   .iMP  ( iMP[23:16]),                      //  Multiplier   operand to multiply by
   .iBS  ( LPo[31:24]),                      //  Byte Shift in
   .ldBW (load),                             //  load BW multiplier
   .signd(signd),                            //  signed\unsigned
   .shft (sBS),                              //  Byte shift
   .last (last),
   .lo   (lo[2]),                            //  multiplier serial out
   .li   (li[2]),                            //  multiplier serial in
   .uo   (uo[2]),                            //  upper sum serial out
   .ui   (ui[2]),                            //  upper sum serial in
   .mb   (mb[2]),                            //  multiply bit to partial product generation
   .ms   (ms[2]),                            //  most significant bits for each size of multiply
   .clock(fast_clock)
   );

assign
   li[2] =  fn[if8]  ? uo[2]  : lo[3],
   ui[2] =  fn[if8]  ? 1'b0   : uo[3],
   mb[2] =  fn[if32] ? lo[0]  : lo[2],
   ms[2] =  fn[if8];

A2_BW_multiply  slice3 (
   .LPo  ( LPo[31:24]),                      //  Lower Product: Integer result of multiply
   .UPPo (UPPo[31:24]),                      //  Upper Partial Product
   .UPCo (UPCo[31:24]),                      //  Upper Partial Carry
   .lo   (lo[3]),                            //  lower sum serial out \ multiplier serial out
   .uo   (uo[3]),                            //  upper sum serial out
   .iMC  ( iMC[31:24]),                      //  Multiplicand Operand to be multiplied
   .iMP  ( iMP[31:24]),                      //  Multiplier   operand to multiply by
   .iBS  ( iBS[ 7:0 ]),                      //  Byte shift input (Output is from LPo)
   .ldBW (load),                             //  load BW multiplier
   .signd(signd),                            //  signed\unsigned
   .shft (sBS),                              //  Byte shift
   .last (last),                             //  Last cycle
   .li   (li[3]),                            //  lower sum serial in  \ multiplier serial in
   .ui   (ui[3]),                            //  upper sum serial in
   .mb   (mb[3]),                            //  multiply bit to partial product generation
   .ms   (ms[3]),                            //  most significant bits for each size of multiply
   .clock(fast_clock)
   );

assign
   li[3] =  fn[if8]  ?  uo[3]
         :  fn[if16] ?  uo[2] : uo[0],
   ui[3] =  1'b0,
   mb[3] =  fn[if8]  ?  lo[3]
         :  fn[if16] ?  lo[2] : lo[0],
   ms[3] =  1'b1;

endmodule

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

module   A2_BW_multiply  (
   output wire [7:0] LPo,                    //  Lower Product: Integer result of multiply
   output wire [7:0] UPPo,                   //  Upper Partial Product
   output wire [7:0] UPCo,                   //  Upper Partial Carry
   input  wire [7:0] iMC,                    //  Multiplicand Operand to be multiplied
   input  wire [7:0] iMP,                    //  Multiplier   operand to multiply by
   input  wire [7:0] iBS,                    //  Shift input
   input  wire       ldBW,                   //  load BW multiplier
   input  wire       signd,                  //  signed\unsigned
   input  wire       shft,
   input  wire       last,
   // Expansion
   output wire       lo, uo,                 //  multiplier serial out  &  upper sum serial out
   input  wire       li, ui,                 //  multiplier serial in   &  upper sum serial in
   input  wire       mb, ms,                 //  multiply bit to partial product generation,  most significant position
   // Global
   input  wire       clock
   );

localparam  if8   = 0,  if16 = 1,   if32 = 2,   sign = 3;

// Locals -----------------------------------------------------------------------------------------------------------------------
//integer  i;

// Physical Registers
reg   [7:0] mcand;
reg   [7:0] lower;
reg   [7:0] upper;
reg   [7:0] carry;
reg         Signd;
reg         run;
// Nets
wire  [7:0] pp, pps, ppl, s, k;
wire        sdlt, sdms;

// ------------------------------------------------------------------------------------------------------------------------------

always @(posedge clock)
   if (ldBW) begin
      mcand <= `tCQ iMC[7:0];
      lower <= `tCQ iMP[7:0];
      upper <= `tCQ 8'h00;
      carry <= `tCQ 8'h00;
      Signd <= `tCQ signd;
      run   <= `tCQ 1'b1;
      end
   else if (run && shft) begin
      lower <= `tCQ  iBS;                                           // Shift bytes through
      run   <= `tCQ !last;                                          // run to completion
      end
   else if (run && !shft) begin
      lower <= `tCQ {li, lower[7:1]};                               // Shift in result bit and shift out multiply bit (mo)
      upper <= `tCQ {Signd && ms ? last : ui,  s[7:1]};             // Shift in redundant sum  shift out result bit   (so)
      carry <= `tCQ  k;                                             // Save redundnt carry
      run   <= `tCQ !last;                                          // run to completion
      end

assign
   sdlt  =  Signd  &  last,                                    // SigneD and LasT
   sdms  =  Signd  &  ms,                                      // SigneD and Most Signigicant slice based on size
   pp    = {8{mb}} &  mcand[7:0],                              // partial product
   pps   = {pp[7]  ^  sdms, pp[6:0]},                          // partial product sign flipped          (BW algo)
   ppl   =   sdlt  ? ~pps  :  pps,                             // partial product flipped for last line (BW algo)

   s     =  upper ^ ppl ^ carry,                               // Redundant sum
   k     =  upper & ppl | carry & upper | ppl & carry,         // Redundant carry

   uo    =  s[0],                                              // Sum bit 0 shifted out. if slice 0 product to lower
   lo    =  lower[0],                                          // multiply bit to partial product generation

    LPo  = lower,                                              // Complete lower product
   UPPo  = upper,                                              // Redundant sum upper product
   UPCo  = carry;                                              // Redundant carry upper product
endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//             /////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// MODULE TEST /////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//             /////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

// Set a command line `define to "test_BW_multiply" to do a standalone module test on this file
`ifdef   test_BW_multiply_BSW

`define  tick  10
//`define  full_display

module t;

parameter   WIDTH       =  32;

parameter   MASK        = ~(32'h ffffffff <<(2*32));
parameter   LOOP        =  1000;
parameter   HEARTBEAT   =  LOOP / 10;        // progress indicators
parameter   TRUE        =  1;
parameter   FALSE       =  0;

reg  [31:0]   iMC;                 //  Multiplicand Operand to be multiplied
reg  [31:0]   iMP;                 //  Multiplier   operand to multiply by
reg  [31:0]   iV2;                 //  Swoozle upper half!
reg           ldBW;                //  load BW multiplier
reg   [4:0]   fn;                  //  signed\unsigned  bit-2: 32-bit, bit-1: 16-bit, bit-0: 8-bit -- one-hot
reg           slow_clock, fast_clock;
reg           reset;

wire [31:0]   LPo;                 //  Lower Product: Integer result of multiply
wire [31:0]   UPPo;                //  Upper Partial Product
wire [31:0]   UPCo;                //  Upper Partial Carry
wire          done;                //  Output is valid

wire [63:0]   product32;           //
wire [31:0]   product16 [0:1];     //
wire [15:0]   product8  [0:3];     //
reg  [63:0]   expected32;          //
reg  [31:0]   expected16[0:1];     //
reg  [15:0]   expected8 [0:3];     //

integer  i,j;
integer  errorcount, testcount, testnum;


A2_BWmultiply_BSW  mut (           //  BSW:  Byte\Short\Word
   .LPo        ( LPo[31:0]),       //  Lower Product: Integer result of multiply
   .UPPo       (UPPo[31:0]),       //  Upper Partial Product
   .UPCo       (UPCo[31:0]),       //  Upper Partial Carry
   .done       (done),             //  Output is valid
   .iMC        ( iMC[31:0]),       //  Multiplicand Operand to be multiplied
   .iMP        ( iMP[31:0]),       //  Multiplier   operand to multiply by
   .iBS        (swoozle[7:0]),     //  Byte shift input
   .ldBW       (ldBW),             //  load BW multiplier
   .fn         (fn[4:0]),          //  bit-4: shift, bit-3: signed\unsigned, bit-2: 32-bit, bit-1: 16-bit, bit-0: 8-bit -- 1-hot
   .setup      (32'h0043_3632),
   .fast_clock (fast_clock),
   .slow_clock (slow_clock),
   .reset      (reset)
   );

wire  [2:0] select   =  LPo[2:0];
wire  [7:0] swoozle  = {iV2[31:0], iMC[31:0]} >> (8 * select[2:0]);  // Very high speed!!!

assign product32[63:00] = {UPPo[31:00] + UPCo[31:00] + (fn[3] ? 1'b1 : 1'b0), LPo[31:00]},
       product16[0]     = {UPPo[15:00] + UPCo[15:00] + (fn[3] ? 1'b1 : 1'b0), LPo[15:00]},
       product16[1]     = {UPPo[31:16] + UPCo[31:16] + (fn[3] ? 1'b1 : 1'b0), LPo[31:16]},
       product8[0]      = {UPPo[07:00] + UPCo[07:00] + (fn[3] ? 1'b1 : 1'b0), LPo[07:00]},
       product8[1]      = {UPPo[15:08] + UPCo[15:08] + (fn[3] ? 1'b1 : 1'b0), LPo[15:08]},
       product8[2]      = {UPPo[23:16] + UPCo[23:16] + (fn[3] ? 1'b1 : 1'b0), LPo[23:16]},
       product8[3]      = {UPPo[31:24] + UPCo[31:24] + (fn[3] ? 1'b1 : 1'b0), LPo[31:24]};

initial begin
   slow_clock = 0;
   forever slow_clock = #(5*`tick) ~slow_clock;
   end

initial begin
   fast_clock = slow_clock;
   forever fast_clock = #(`tick) ~fast_clock;
   end


wire  irdy  =  ldBW ^ done;

// TESTS UNSIGNED
// ffff * ffff = fffe0001
// 8000 * 8000 = 40000000
// 0800 * 0018 = 0000c000
// 0800 * 0020 = 00010000
// TESTS SIGNED
// ffff * ffff = 00000001
// 8000 * 8000 = 40000000
// ff80 * ff88 = 00003c00
// f800 * f801 = 003ff800

   reg   [34:0]   a,b,c;
   reg   [37:0]   d,e,f;



initial begin
   $display("\n BAUGH-WOOLEY BYTE\SHORT\WORD MULTIPLIER  \n");
   errorcount  =  0;
   testcount   =  0;
   ldBW        =  0;
   reset       =  0;
   repeat(10)   @(posedge slow_clock);
   reset       =  1;
   repeat(1)    @(posedge slow_clock);
   reset       =  0;
   repeat(10)   @(posedge slow_clock);

   $display(" BEGIN DIRECTED TESTS \n");

   // --------------------------------------------------------------------------------------------------------
   testnum     =  1;
   $display(" Test %2d  4 x  8x8  unsigned multiplies ",testnum);
   repeat(1)     @(posedge slow_clock); #1;
   iMC   = 32'hff_ff_ff_ff;
   iMP   = 32'hff_ff_ff_ff;
   fn    =  5'b00001;             // 8-bit unsigned
   ldBW  =  1'b1;
   repeat(1)     @(posedge slow_clock);
   ldBW  =  1'b0;
   repeat(1)     @(posedge slow_clock);

   while (!done) @(posedge slow_clock); #1;
   for (i=0;i<4;i=i+1) begin
      expected8[i] = iMC[8*i+:8] * iMP[8*i+:8];
      if (product8[i] !== expected8[i]) begin
                                        $display("  trial %2d.%1d failed: %h * %h = should be %h; found %h",
                                          testnum,i,iMC[8*i+:8],iMP[8*i+:8],expected8[i],product8[i]);
                                        errorcount = errorcount +1;
                                        end
      end
   // --------------------------------------------------------------------------------------------------------
   testnum     =  2;
   $display(" Test %2d  2 x 16x16 unsigned multiplies ",testnum);
   repeat(1)     @(posedge slow_clock);
   iMC   = 32'h0000_ffff;
   iMP   = 32'h0000_ffff;
   fn    =  5'b00010;             // 16-bit unsigned
   ldBW  =  1'b1;
   repeat(1)     @(posedge slow_clock);
   ldBW  =  1'b0;
   repeat(1)     @(posedge slow_clock);

   while (!done) @(posedge slow_clock); #1;
   for (i=0;i<2;i=i+1) begin
      expected16[i] = iMC[16*i+:16] * iMP[16*i+:16];
      if (product16[i] !== expected16[i]) begin
                                          $display("  trial %2d.%1d failed: %h * %h = should be %h; found %h",
                                             testnum,i,iMC[16*i+:16],iMP[16*i+:16],expected16[i],product16[i]);
                                          errorcount = errorcount +1;
                                          end
      end

   // --------------------------------------------------------------------------------------------------------
   testnum     =  3;
   $display(" Test %2d  1 x 32x32 unsigned multiply ",testnum);
   repeat(1)     @(posedge slow_clock);
   trial ( 32'hffffffff, 32'hffffffff, 5'b00100);

   // --------------------------------------------------------------------------------------------------------
   testnum     =  4;
   $display(" Test %2d  4 x  8x8    signed multiplies ",testnum);
   repeat(1)   @(posedge slow_clock);
   trial ( 32'hffffffff, 32'hffff00ff, 5'b01001);
   // --------------------------------------------------------------------------------------------------------
   testnum     =  5;
   $display(" Test %2d  2 x 16x16   signed multiplies ",testnum);
   repeat(1)   @(posedge slow_clock);
   trial ( 32'h0001ffff, 32'hffffffff, 5'b01010);
   // --------------------------------------------------------------------------------------------------------
   testnum     =  6;
   $display(" Test %2d  1 x 32x32   signed multiply ",testnum);
   repeat(1)   @(posedge slow_clock);
   trial ( 32'hffffffff, 32'hffffffff, 5'b01100);
   // --------------------------------------------------------------------------------------------------------
   testnum     =  7;
   $display(" Test %2d  4 x  8x8  unsigned multiplies ",testnum);
   repeat(1)   @(posedge slow_clock);
   trial ( 32'h80808080, 32'h80808080, 5'b00001);
   // --------------------------------------------------------------------------------------------------------
   testnum     =  8;
   $display(" Test %2d  2 x 16x16 unsigned multiplies ",testnum);
   repeat(1)   @(posedge slow_clock);
   trial ( 32'h80008000, 32'h80008000, 5'b00010);
   // --------------------------------------------------------------------------------------------------------
   testnum     =  9;
   $display(" Test %2d  1 x 32x32 unsigned multiply ",testnum);
   repeat(1)   @(posedge slow_clock);
   trial ( 32'h80000000, 32'h80000000, 5'b00100);
   // --------------------------------------------------------------------------------------------------------
   testnum     = 10;
   $display(" Test %2d  4 x  8x8    signed multiplies ",testnum);
   repeat(1)   @(posedge slow_clock);
   trial ( 32'h80808080, 32'h80808080, 5'b01001);
   // --------------------------------------------------------------------------------------------------------
   testnum     = 11;
   $display(" Test %2d  2 x 16x16   signed multiplies ",testnum);
   repeat(1)   @(posedge slow_clock);
   trial ( 32'h80008000, 32'h80008000, 5'b01010);
   // --------------------------------------------------------------------------------------------------------
   testnum     = 12;
   $display(" Test %2d  1 x 32x32   signed multiply ",testnum);
   repeat(1)   @(posedge slow_clock);
   trial ( 32'h80000000, 32'h80000000, 5'b01100);

   // --------------------------------------------------------------------------------------------------------
   testnum     = 13;
   $display(" Test %2d  4 x 8x8   signed multiply ",testnum);
   repeat(1)   @(posedge slow_clock);
   trial ( 32'h80c0e0f0, 32'h80c0e0f0, 5'b01001);

   // --------------------------------------------------------------------------------------------------------
   testnum     = 14;
   $display(" Test %2d  1 x 32x32   shift select ",testnum);
   repeat(1)   @(posedge slow_clock); #1;
   iV2   =  32'hffeeddcc;
   iMC   =  32'h73625140;
   iMP   =  32'h03020100;
   fn    =   5'b10000;
   ldBW  =   1'b1;
   repeat(1)   @(posedge slow_clock); #1;
   ldBW  =   1'b0;
   repeat(1)     @(posedge slow_clock); #1;
   while (!done) @(posedge slow_clock); #1;
      expected32 =  32'h73625140;
   if (product32 !== expected32)   begin
      $display("  trial %2d.0 failed: %h * %h = should be %h; found %h",
                   testnum,iMC[32*i+:32],iMP[32*i+:32],expected32,product32);
      errorcount = errorcount +1;
      end

   repeat(100)    @(posedge slow_clock);
   // --------------------------------------------------------------------------------------------------------
   if (errorcount == 1) $display("\n ALL TESTS DONE.... with  1 error   \n");
   else                 $display("\n ALL TESTS DONE.... with %2d errors \n", errorcount);
   $stop();
   $finish();
   end

// ------------------------------------------------------------------------------------------------------------------------------

task trial;
   input [31:0]   multiplicand;
   input [31:0]   multiplier;
   input [ 4:0]   mulfunction;
   iMC   = multiplicand;
   iMP   = multiplier;
   fn    = mulfunction;
   ldBW  =  1'b1;
   repeat(1)     @(posedge slow_clock);
   ldBW  =  1'b0;
   repeat(1)     @(posedge slow_clock);
   while (!done) @(posedge slow_clock); #1;
   case (1'b1)
      fn[0] :  for (i=0;i<4;i=i+1) begin
                  if (fn[3]) expected8[i] = $signed(iMC[8*i+:8]) * $signed(iMP[8*i+:8]);
                  else       expected8[i] =         iMC[8*i+:8]  *         iMP[8*i+:8];
                  if (product8[i] !== expected8[i]) begin
                     $display("  trial %2d.%1d failed: %h * %h = should be %h; found %h",
                                  testnum,i,iMC[8*i+:8],iMP[8*i+:8],expected8[i],product8[i]);
                     errorcount = errorcount +1;
                     end
                  $display(" %h * %h = {(%h + %h + %b),%h} => %4h <= %4h",
                     iMC[8*i+:8],
                     iMP[8*i+:8],
                     UPPo[8*i+:8],
                     UPCo[8*i+:8],
                     fn[3],
                     LPo[8*i+:8],
                     product8[i],
                    (fn[3] ? ($signed(iMC[8*i+:8]) * $signed(iMP[8*i+:8])) : (iMC[8*i+:8] * iMP[8*i+:8])));

                  end

      fn[1] :  for (i=0;i<2;i=i+1) begin
                  if (fn[3]) expected16[i] = $signed(iMC[16*i+:16]) * $signed(iMP[16*i+:16]);
                  else       expected16[i] =          iMC[16*i+:16] *         iMP[16*i+:16];
                  if (product16[i] !== expected16[i]) begin
                     $display("  trial %2d.%1d failed: %h * %h = should be %h; found %h",
                                 testnum,i,iMC[16*i+:16],iMP[16*i+:16],expected16[i],product16[i]);
                     errorcount = errorcount +1;
                     end
                  end

      fn[2] :  begin
               if (fn[3])  expected32 = $signed(iMC[31:0]) * $signed(iMP[31:0]);
               else        expected32 =         iMC[31:0]  *         iMP[31:0];
               if (product32 !== expected32)   begin
                  $display("  trial %2d.0 failed: %h * %h = should be %h; found %h",
                               testnum,iMC[32*i+:32],iMP[32*i+:32],expected32,product32);
                  errorcount = errorcount +1;
                  end
               end
      default  $display("  OOPS ??? ");
      endcase
   endtask


//   // Double bit Separation
//   d = 38'b01010101_00_01010101_10_11111111_00_11111111;
//   e = 38'b00001010_00_00001010_10_11111111_00_11111111;
//   f = d + e;
//   $display("result08 = %h %h %h %h", f[37:30],f[27:20],f[17:10],f[7:0]);
//
//   d = 38'b00000111_11_11111111_10_00000111_11_11111111;
//   e = 38'b00000000_11_00000000_10_00000000_00_00000001;
//   f = d + e;
//   $display("result16 = %h%h %h%h", f[37:30],f[27:20],f[17:10],f[7:0]);
//
//   d = 38'b11111111_11_11111111_11_11111111_11_11111111;
//   e = 38'b11111111_11_11111111_00_11111111_00_11111111;
//   f = d + e;
//   $display("result32 = %h%h%h%h", f[37:30],f[27:20],f[17:10],f[7:0]);
//
//   // Double bit Separation with carry in
//   d = 38'b11111111_10_11111111_00_11111111_10_11111111;
//   e = 38'b11111111_10_11111111_00_11111111_10_11111111;
//   f = d + e;
//   $display("result08 = %h %h %h %h", f[37:30],f[27:20],f[17:10],f[7:0]);
////                 join        cin         join
//   d = 38'b11111111_11_11111111_10_11111111_11_11111111;
//   e = 38'b11111111_00_11111111_10_11111111_00_11111111;
//   f = d + e;
//   $display("result16 = %h%h %h%h", f[37:30],f[27:20],f[17:10],f[7:0]);
//
//   d = 38'b11111111_11_11111111_11_11111111_11_11111111;
//   e = 38'b11111111_11_11111111_11_11111111_11_11111111;
//   f = d + e + 1;
//   $display("result32 = %h%h%h%h", f[37:30],f[27:20],f[17:10],f[7:0]);
//
//// So  interbyte doublets  00    separate bytes
////                         11    join     bytes
////                         10    separate bytes and add carry in


endmodule

`endif

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////


