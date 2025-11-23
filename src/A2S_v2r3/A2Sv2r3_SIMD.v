//===============================================================================================================================
//
//   Copyright © 2024 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to Authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name         : A2S  WASM SIMD Extension
//
//   Description          : 128-bit SIMD engine using WASM opcodes plus a few of our own
//
//===============================================================================================================================
//   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//
//   The A2S ISA extension for WASM-SIMD and a2-simd are overlaid and 9th bit included in the Function code to distinguish them.
//   The WASM SIMD uses only 1 major code from the A2S extension layout even though only a few WASM-SIMD codes use a short
//   immediate.  So we use 0xe and a 12-bit field that is a 4-bit short immediate and an 8bit opcode.
//   The a2-simd does not use an extra immediate field and is always always zero.  The transformation is done in the A2S
//   extensions module prior to output to this module.
//
//===============================================================================================================================
//   Additional instructions to ease emulation of WASM-SIMD:
//     lt_s_push      :  V3 V2 V1 V0 => V2 V1 V0 (V1<V0)     when followed by bitselect => min/max/sat based in initial V1, V2
//     lt_u_push      :  V3 V2 V1 V0 => V2 V1 V0 (V1<V0)     as above
//     avg            :  V3 V2 V1 V0 => V3 V3 V2 {V1,V0}>>1  average!
//===============================================================================================================================

  `define  ENABLE_NARROW
//`define  ENABLE_DOT_PRODUCT       // Not implemented correctly!! -- don't use
  `define  ENABLE_NEGABSSAT
`ifdef synthesis
`include "/proj/TekStart/lokotech/soc/users/romeo/newport_src_a0/src/Top/NEWPORT_project_settings.v"
`endif

//`define  STANDALONE
`ifdef   STANDALONE
   `define  IMISO_WIDTH    33
   `define  IMOSI_WIDTH    48
   `define  PMISO_WIDTH    35
   `define  PMOSI_WIDTH    47
   `define  WMISO_WIDTH   130
   `define  WMOSI_WIDTH   290
   `define  tCQ           #1
`endif   //STANDALONE

// MAIN MODULE ==================================================================================================================

module A2Sv2r3_SIMD #(
   parameter BASE       =  13'h0000,               // Register Address Base
   parameter NUM_REGS   =  6
   )(
   // imiso/imosi interface
   output wire [`IMISO_WIDTH-1:0]   imiso,         // I/O data   to  A2S    33 {IOk, IOo[31:0]}
   input  wire [`IMOSI_WIDTH-1:0]   imosi,         // I/O data  from A2S    48 {cIO[2:0] aIO[12:0], iIO[31:0]}
   // A2S SIMD inteface outputs
   output wire [`PMISO_WIDTH-1:0]   pmiso,         // SIMD Port  to  A2S    35 { SIMDir,SIMDor,SIMDss,     SIMDex,     SIMDo[31:0]}
   input  wire [`PMOSI_WIDTH-1:0]   pmosi,         // SIMD Port from A2S    47 { eSIMD, tSIMD, cSIMD[8:0], mSIMD[3:0],iSIMD [31:0]}
   // WM Interface
   output wire [`WMOSI_WIDTH-1:0]   wmosi,         // SIMD Port  to  WRAM  290 {eWM, wWM, iWM[127:0], mWM[127:0], aWM[31:0]}
   input  wire [`WMISO_WIDTH-1:0]   wmiso,         // SIMD port from WRAM  130 {WMg, WMk, WMo[127:0]}
   // Globals
   input  wire                      reset,
   input  wire                      fast_clock,
   input  wire                      clock
   );

`ifdef   RELATIVE_FILENAMES                    // DEFINE IN SIMULATOR COMMAND LINE
   `include "A2Sv2r3_ISA.vh"                   // localparams defining ISA opcodes
`elsif   STANDALONE
   localparam  [2:0]
      index_SIMD_CT  = 3'd0,
      index_SIMD_CF  = 3'd1,
      index_SIMD_WP  = 3'd2,
      index_SIMD_XP  = 3'd3,
      index_SIMD_YP  = 3'd4,
      index_SIMD_ZP  = 3'd5;
`else
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2Sv2r3_ISA.vh"                   // localparams defining ISA opcodes
`endif //STANDALONE

// CONTROL_PARAMETERS -----------------------------------------------------------------------------------------------------------
localparam  [3:0]    hold = 4'd0,    push  = 4'd1,    pop  = 4'd2,    swap = 4'd3,
                     urt4 = 4'd4,    nip   = 4'd5,    rot3 = 4'd6,    rot4 = 4'd7,
                     exe1 = 4'd8,    exe2  = 4'd9,    exe3 = 4'd10,   exe4 = 4'd11,
                     exe5 = 4'd12,   exe6  = 4'd13,   exe7 = 4'd14,   fill = 4'd15;
localparam  [3:0]    lbV0 = 4'b1010, lbV1  = 4'b1100, lnot = 4'b0101,
                     land = 4'b1000, lxor  = 4'b0110, lior = 4'b1110, lann = 4'b0100;
localparam  [1:0]    hldS = 2'd0,    pushS = 2'd1,    popS = 2'd2;
localparam  [1:0]    no   = 2'd0,    rd    = 2'd1,    wr   = 2'd2;
localparam  [1:0]    Byte = 0,       Shrt  = 1,       Word = 2;
localparam  [1:0]    Right= 0,       Arith = 1,       Left = 2;
//-------------------------------------------------------------------------------------------------------------------------------

// Locals =======================================================================================================================

integer        i,j;
genvar         g;

wire  [127:0]  V0, V1, V2, V3, EN;              // Vector Stack V0..V3, Memory Write Data & Memory bit mask ENable
wire  [127:0]  MR;                              // Memory Read data
wire  [127:0]  SHF;                             // Shifter Output

wire  [127:0]  mask;                            // Memory mask expanded from 'eLane'

wire  [31:0]   Sw, Sc;                          // Scalar results
wire           illegal_opcode_wasm;             // Unused Opcode!!
wire           illegal_opcode_simd;             // Unused Opcode!!
wire           setEN, loadEN, writeEN, flipEN;  // Bit-level Memory Mask ENables
wire           ENwasm, ENsimd, stall;

wire           read_next, change, write_next, serial_op, scalar_out;

// Early Recode multiply functions
wire    [4:0]  emulfn;                          // Early decodes
wire           mul;
reg     [4:0]  MULFN;                           // Registered Early decodes
reg            LDBW, MUL;

// Early decodes -- done during incoming cycle
wire           pw8, pw8s, pw16, pw16s, enB;
reg            PW8, PW8s, PW16, PW16s, ENB, SUBT;

// A2S interface
wire  [31:0]   iSIMD;   // Incoming Scalar data
wire  [ 8:0]   cSIMD;   // Incoming Command -- SIMD instructions
wire  [ 3:0]   mSIMD;   // Small iMmediates
wire           eSIMD;   // Enable (push if input  ready)
wire           tSIMD;   // Take   (pop  if output ready)

reg   [ 8:0]   FN;      // SIMD Function
reg   [ 3:0]   IMM;     // SIMD Immediate -- usually lane number
reg   [31:0]   iS;      // Registered incoming Scalar

wire           done;    // End of multicycle operation

wire  [31:0]   SIMDo;   // Registered Outgoing Scalar
wire           SIMDir;  // SIMD input ready
wire           SIMDex;  // SIMD exception
wire           SIMDor;  // SIMD output ready

// WM Interface
wire  [127:0]  iWM, mWM;
reg   [ 31:0]  advance,address;
wire  [  1:0]  Mctrl;
reg            eWM, wWM;
wire           grant, wack, mrak;


// imiso/imosi Interface decoding ===============================================================================================

`ifdef   STANDALONE
   localparam  LNRG  =   3;
`else
   localparam  LNRG  =  `LG2(NUM_REGS);
`endif //STANDALONE

wire  [31:0]   iIO, IOo;
wire  [12:0]   aIO;
wire  [ 2:0]   cIO;
wire                IOk;

assign   {cIO[2:0], iIO[31:0], aIO[12:0]} = imosi;

wire                 imosi_wr = cIO==3'b110;
wire                 imosi_rd = cIO==3'b101;
wire                 in_range = aIO[12:LNRG] == BASE[12:LNRG];
wire  [NUM_REGS-1:0]   decode = in_range << aIO[LNRG-1:0];
wire  [NUM_REGS-1:0]    write = {NUM_REGS{imosi_wr}} & decode;
wire  [NUM_REGS-1:0]     read = {NUM_REGS{imosi_rd}} & decode;

//Collate imiso
assign   imiso = {IOk, IOo};

// IO Registers =================================================================================================================

localparam  [1:0] sWP = 0, sXP = 1, sYP = 2, sZP = 3;

reg   [31:0]   WP, XP, YP, ZP, CT;
(* dont_touch *) reg   [31:0]  CF;
wire  [31:0]   aWM;
reg   [ 3:0]   update;

// W Pointer : Memory pointer with post increment reads and pre-decrement writes
always @(posedge clock)
   if       (  reset      ) WP <= `tCQ  32'h0;
   else if  (  write[`iWP]) WP <= `tCQ  iIO[31:0];
   else if  ( update[ sWP]) WP <= `tCQ  advance[31:0];

// X Pointer : Memory pointer with post increment reads and writes
always @(posedge clock)
   if       (  reset      ) XP <= `tCQ  32'h0;
   else if  (  write[`iXP]) XP <= `tCQ  iIO[31:0];
   else if  ( update[ sXP]) XP <= `tCQ  advance[31:0];

// Y Pointer : Memory pointer with post increment reads and writes
always @(posedge clock)
   if       (  reset      ) YP <= `tCQ  32'h0;
   else if  (  write[`iYP]) YP <= `tCQ  iIO[31:0];
   else if  ( update[ sYP]) YP <= `tCQ  advance[31:0];

// Z Pointer : Memory pointer with post increment reads and writes
always @(posedge clock)
   if       (  reset      ) ZP <= `tCQ  32'h0;
   else if  (  write[`iZP]) ZP <= `tCQ  iIO[31:0];
   else if  ( update[ sZP]) ZP <= `tCQ  advance[31:0];

// Enable : Turns on the clocks for the rest of SIMD
always @(posedge clock)
   if       (  reset      ) CT <= `tCQ  32'h0;         // Resoundingly OFF !
   else if  (  write[`iCT]) CT <= `tCQ  iIO[31:0];

// Configures the slow to fast clock domain crossings and performance
always @(posedge clock)
   if       (  reset      ) CF <= `tCQ  32'h0;         // Resoundingly OFF !
   else if  (  write[`iCF]) CF <= `tCQ  iIO[31:0];

A2_mux #(6,32) iIOO (.z(IOo[31:0]), .d({ CF, CT, ZP, YP, XP, WP }), .s(read[NUM_REGS-1:0]));

wire  any_read =  |read[NUM_REGS-1:0];
wire  any_wryt =  |write[NUM_REGS-1:0];
assign   IOk   =   any_read | any_wryt;

// A2S Interface ================================================================================================================

// Decollate
assign   {eSIMD, tSIMD, cSIMD[8:0], mSIMD[3:0], iSIMD [31:0]}= pmosi[`PMOSI_WIDTH-1:0];

// Pre-decodes
assign read_next  = eSIMD & ( cSIMD[8] ?  cSIMD[7:0] == v128_load                   // Reads are a  2-cycle operation that may be
                                       : (cSIMD[7:4] == simd_push         [7:4]     // overlapped so consecutive reads look
                                       |  cSIMD[7:4] == simd_replace      [7:4]     // like a burst and so are treated as such
                                       |  cSIMD[7:4] == simd_popcount     [7:4]
                                       |  cSIMD[7:4] == simd_xnor_popcount[7:4]
                                       |  cSIMD[7:4] == simd_load_EN      [7:4] ));

assign change     = eSIMD & ( cSIMD[8] ? (cSIMD[7:0] != FN[7:0])                    // This could be done better.  If the next
                                       : (cSIMD[7:4] != FN[7:4]));                  // is another read then we should not change!

assign write_next = eSIMD & ( cSIMD[8] ?  cSIMD[7:0] == v128_store
                                       : (cSIMD[7:4] == simd_store_EN    [7:4]
                                       |  cSIMD[7:4] == simd_store_masked[7:4]
                                       |  cSIMD[7:4] == simd_store       [7:4] ));

assign serial_op  = eSIMD & ( cSIMD[8] & (cSIMD[7:0] == i16x8_mul
                                       |  cSIMD[7:0] == i32x4_mul
                                       |  cSIMD[7:0] == i16x8_extmul_dbl_i8x16_s
                                       |  cSIMD[7:0] == i16x8_extmul_dbl_i8x16_u
                                       |  cSIMD[7:0] == i32x4_extmul_dbl_i16x8_s
                                       |  cSIMD[7:0] == i32x4_extmul_dbl_i16x8_u
                                       |  cSIMD[7:0] == i8x16_swizzle
                                       |  cSIMD[7:0] == i8x16_shuffle));

assign scalar_out = eSIMD &  ~cSIMD[8] & (cSIMD[7:0] == simd_store_scalar)
                  | eSIMD &  ~cSIMD[8] & (cSIMD[7:0] == simd_vector_sum)
                  | eSIMD &   cSIMD[8] & (cSIMD[7:0] == v128_any_true)
                  | eSIMD &   cSIMD[8] & (cSIMD[7:0] == i8x16_bitmask)
                  | eSIMD &   cSIMD[8] & (cSIMD[7:0] == i16x8_bitmask)
                  | eSIMD &   cSIMD[8] & (cSIMD[7:0] == i32x4_bitmask)
                  | eSIMD &   cSIMD[8] & (cSIMD[7:0] == i8x16_extract_lane_s)
                  | eSIMD &   cSIMD[8] & (cSIMD[7:0] == i16x8_extract_lane_s)
                  | eSIMD &   cSIMD[8] & (cSIMD[7:0] == i32x4_extract_lane)
                  | eSIMD &   cSIMD[8] & (cSIMD[7:0] == i8x16_extract_lane_u)
                  | eSIMD &   cSIMD[8] & (cSIMD[7:0] == i16x8_extract_lane_u)
                  | eSIMD &   cSIMD[8] & (cSIMD[7:0] == i8x16_all_true)
                  | eSIMD &   cSIMD[8] & (cSIMD[7:0] == i16x8_all_true)
                  | eSIMD &   cSIMD[8] & (cSIMD[7:0] == i32x4_all_true);

// State Machine ----------------------------------------------------------------------------------------------------------------
reg [7:0] caseS;   // debug only
reg [2:0] FSM; localparam [2:0]  IDLE = 0, RDRQ = 1, RDBU = 2, RDND = 3, SORQ = 4, MCYC = 5, BUSY = 6;
                                //default
always @(posedge clock)
   if (reset)                   begin {FSM[2:0],FN[8:0],IMM[3:0]}    <= `tCQ {IDLE, 9'h0,       4'h0      }; caseS <=  0; end //JLSW
   else (* parallel_case *)  case     (FSM[2:0])
      IDLE  :  if ( read_next ) begin {FSM[2:0],FN[8:0]         }    <= `tCQ {RDRQ, cSIMD[8:0]            }; caseS <=  1; end //JLSW
          else if ( serial_op ) begin {FSM[2:0],FN[8:0]         }    <= `tCQ {MCYC, cSIMD[8:0]            }; caseS <=  2; end //JLSW
          else if ( scalar_out) begin {FSM[2:0],FN[8:0],IMM[3:0]}    <= `tCQ {SORQ, cSIMD[8:0], mSIMD[3:0]}; caseS <=  3; end //JLSW
          else if ( eSIMD     ) begin {FSM[2:0],FN[8:0],IMM[3:0]}    <= `tCQ {BUSY, cSIMD[8:0], mSIMD[3:0]}; caseS <=  4; end //JLSW

      RDRQ :   if ( read_next ) begin {FSM[2:0],FN[8:0],IMM[3:0]}    <= `tCQ {RDBU, cSIMD[8:0], mSIMD[3:0]}; caseS <=  5; end   // Read Request //JLSW
          else                  begin  FSM[2:0]                      <= `tCQ  RDND;                          caseS <=  6; end //JLSW

      RDBU :   if ( read_next ) begin {FSM[2:0],FN[8:0],IMM[3:0]}    <= `tCQ {RDBU, cSIMD[8:0], mSIMD[3:0]}; caseS <=  7; end   // Read Burst //JLSW
          else                  begin  FSM[2:0]                      <= `tCQ  RDND;                          caseS <=  8; end //JLSW

      RDND :   if ( read_next ) begin {FSM[2:0],FN[8:0],IMM[3:0]}    <= `tCQ {RDRQ, cSIMD[8:0], mSIMD[3:0]}; caseS <=  9; end //JLSW
          else if ( serial_op ) begin {FSM[2:0],FN[8:0]         }    <= `tCQ {MCYC, cSIMD[8:0]            }; caseS <= 10; end //JLSW
          else if ( scalar_out) begin {FSM[2:0],FN[8:0],IMM[3:0]}    <= `tCQ {SORQ, cSIMD[8:0], mSIMD[3:0]}; caseS <= 11; end //JLSW
          else if ( eSIMD     ) begin {FSM[2:0],FN[8:0],IMM[3:0]}    <= `tCQ {BUSY, cSIMD[8:0], mSIMD[3:0]}; caseS <= 12; end //JLSW
          else                  begin  FSM[2:0]                      <= `tCQ  IDLE;                          caseS <= 13; end //JLSW

      SORQ :                    begin  FSM[2:0]                      <= `tCQ  IDLE;                          caseS <= 14; end //JLSW

      MCYC :   if ( done ) begin
                  if (read_next  ) begin {FSM[2:0],FN[8:0],IMM[3:0]} <= `tCQ {RDRQ, cSIMD[8:0], mSIMD[3:0]}; caseS <= 15; end //JLSW
             else if ( serial_op ) begin {FSM[2:0],FN[8:0]         } <= `tCQ {MCYC, cSIMD[8:0]            }; caseS <= 16; end //JLSW
             else if ( scalar_out) begin {FSM[2:0],FN[8:0],IMM[3:0]} <= `tCQ {SORQ, cSIMD[8:0], mSIMD[3:0]}; caseS <= 17; end //JLSW
             else if ( eSIMD     ) begin {FSM[2:0],FN[8:0],IMM[3:0]} <= `tCQ {BUSY, cSIMD[8:0], mSIMD[3:0]}; caseS <= 18; end //JLSW
             else                  begin  FSM[2:0]                   <= `tCQ  IDLE;                          caseS <= 19; end //JLSW
             end

      BUSY :   if ( read_next ) begin {FSM[2:0],FN[8:0],IMM[3:0]}    <= `tCQ {RDRQ, cSIMD[8:0], mSIMD[3:0]}; caseS <= 20; end //JLSW
          else if ( serial_op ) begin {FSM[2:0],FN[8:0]         }    <= `tCQ {MCYC, cSIMD[8:0]            }; caseS <= 21; end //JLSW
          else if ( scalar_out) begin {FSM[2:0],FN[8:0],IMM[3:0]}    <= `tCQ {SORQ, cSIMD[8:0], mSIMD[3:0]}; caseS <= 22; end //JLSW
          else if ( eSIMD     ) begin {FSM[2:0],FN[8:0],IMM[3:0]}    <= `tCQ {BUSY, cSIMD[8:0], mSIMD[3:0]}; caseS <= 23; end //JLSW
          else                  begin  FSM[2:0]                      <= `tCQ  IDLE;                          caseS <= 24; end //JLSW
      endcase

wire  idle  =  FSM == IDLE,
      rdrq  =  FSM == RDRQ,      // Work Memory Read Enable
      rdbu  =  FSM == RDBU,      // Work Memory Burst Read Enable -- pipelined: finish one and start another!
      rdnd  =  FSM == RDND,      // Work Memory Read End
      sorq  =  FSM == SORQ,      // Scalar Output required
      mcyc  =  FSM == MCYC,      // Multicycle (bit/byte serial) op
      busy  =  FSM == BUSY;      // All other accesses

assign   ENwasm   =  FN[8] & (busy | rdbu | rdnd | sorq );
assign   ENsimd   = ~FN[8] & (busy | rdbu | rdnd | sorq );
assign   stall    =  rdrq  & ~read_next
                  |  rdbu  & ~read_next
                  |  mcyc  & ~done
                  |  scalar_out & ~sorq;

wire     run      =  rdrq  &  read_next
                  |  rdbu  & ~change
                  |  rdnd
                  |  sorq
                  |  mcyc  & done
                  |  busy;

assign   SIMDir   = ~stall;

always @(posedge clock) if  (eSIMD && SIMDir)  iS[31:0] <= `tCQ  iSIMD[31:0];

assign   SIMDex   =  ENwasm & illegal_opcode_wasm | ENsimd & illegal_opcode_simd;
assign   SIMDor   =  sorq;

A2_mux #(2,32) iSo (.z(SIMDo), .s({FN[8],~FN[8]}), .d({Sw[31:0],Sc[31:0]}));

// Collate
assign
   pmiso[34:0] =  { SIMDir,SIMDor,SIMDex, SIMDo[31:0]};     // Input ready, Output ready,  Exception, Scalar output

// WM Accessing =================================================================================================================
reg [3:0]   caseW;  // debug only
wire        enEN;

always @* (* parallel_case *) if (!idle) casez (FN[8:0])
     {5'b00???,WINC} : begin advance = WP + 32'h0000_0010;  address =  WP;      update = {3'b000,eWM    }; caseW =  0; end
     {5'b00???,WPTR} : begin advance = WP;                  address =  WP;      update =  4'b0000;         caseW =  1; end
     {5'b00???,WDEC} : begin advance = WP + 32'hffff_fff0;  address =  advance; update = {3'b000,eWM    }; caseW =  2; end
     {5'b00???,XINC} : begin advance = XP + 32'h0000_0010;  address =  XP;      update = {2'b00,eWM,1'b0}; caseW =  3; end
     {5'b00???,XPTR} : begin advance = XP;                  address =  XP;      update =  4'b0000;         caseW =  4; end
     {5'b00???,XDEC} : begin advance = XP + 32'hffff_fff0;  address =  advance; update = {2'b00,eWM,1'b0}; caseW =  5; end
     {5'b00???,YINC} : begin advance = YP + 32'h0000_0010;  address =  YP;      update = {1'b0,eWM,2'b00}; caseW =  6; end
     {5'b00???,YPTR} : begin advance = YP;                  address =  YP;      update =  4'b0000;         caseW =  7; end
     {5'b00???,YDEC} : begin advance = YP + 32'hffff_fff0;  address =  advance; update = {1'b0,eWM,2'b00}; caseW =  8; end
     {5'b00???,ZINC} : begin advance = ZP + 32'h0000_0010;  address =  ZP;      update = {    eWM,3'b000}; caseW =  9; end
     {5'b00???,ZPTR} : begin advance = ZP;                  address =  ZP;      update =  4'b0000;         caseW = 10; end
     {5'b00???,ZDEC} : begin advance = ZP + 32'hffff_fff0;  address =  advance; update = {    eWM,3'b000}; caseW = 11; end
      default          begin advance = ZP;                  address =  iS;      update =  4'b0000;         caseW = 12; end
      endcase
   else                begin advance = ZP;                  address =  iS;      update =  4'b0000;         caseW = 15; end

always @(posedge clock) if (read_next || write_next || eWM || reset)  eWM <= `tCQ  ~reset & (read_next || write_next || ~eWM);
always @(posedge clock) if (             write_next || wWM || reset)  wWM <= `tCQ  ~reset & (             write_next || ~wWM);

assign
   aWM[31:0]      =   eWM              ?  address  : 32'h0,
   mWM[127:0]     =   ENsimd && enEN   ?  EN       : mask[127:0],
   iWM[127:0]     =   V0[127:0];

// collate output
assign   wmosi[289:0]= {eWM, wWM, iWM[127:0], mWM[127:0], aWM[31:0]};

// decollate input
assign  {grant, wack, MR[127:0]}  =  wmiso[129:0];
assign   mrak     =  (rdbu | rdnd)  &  wack;

// Helper Macros! ===============================================================================================================

`define  BYTES  begin for(i=0; i<16; i=i+1)
`define  SHORTS begin for(i=0; i<8;  i=i+1)
`define  WORDS  begin for(i=0; i<4;  i=i+1)

`define  SS08(x) ($signed(x) <   -128 ?   -128 : $signed(x) >   127 ?   127 : (x)) //  8-bit Signed   Saturate of signed inputs
`define  SS16(x) ($signed(x) < -32768 ? -32768 : $signed(x) > 32767 ? 32767 : (x)) // 16-bit Signed   Saturate of signed inputs
`define  US08(x) ($signed(x) <      0 ?      0 : $signed(x) >   255 ?   255 : (x)) //  8-bit Unsigned Saturate of signed inputs
`define  US16(x) ($signed(x) <      0 ?      0 : $signed(x) > 65535 ? 65535 : (x)) // 16-bit Unsigned Saturate of signed inputs

// EARLY DECODES ================================================================================================================

wire [255:0] pd = 256'b01 << cSIMD[7:0];   // Pre Decode

wire   subt  =
     pd[i8x16_sub  ] | pd[i16x8_sub  ] | pd[i32x4_sub  ]
   | pd[i8x16_eq   ] | pd[i16x8_eq   ] | pd[i32x4_eq   ]
   | pd[i8x16_lt_s ] | pd[i16x8_lt_s ] | pd[i32x4_lt_s ]
   | pd[i8x16_lt_u ] | pd[i16x8_lt_u ] | pd[i32x4_lt_u ]
`ifdef   ENABLE_NEGABSSAT
   | pd[i8x16_abs  ] | pd[i16x8_abs  ] | pd[i32x4_abs  ]
   | pd[i8x16_neg  ] | pd[i16x8_neg  ] | pd[i32x4_neg  ]
   | pd[i8x16_sub_sat_s] | pd[i16x8_sub_sat_s] | pd[i8x16_sub_sat_u] | pd[i16x8_sub_sat_u]
`endif //ENABLE_NEGABSSAT
   | pd[i8x16_max_s] | pd[i16x8_max_s] | pd[i32x4_max_s]
   | pd[i8x16_max_u] | pd[i16x8_max_u] | pd[i32x4_max_u]
   | pd[i8x16_min_s] | pd[i16x8_min_s] | pd[i32x4_min_s]
   | pd[i8x16_min_u] | pd[i16x8_min_u] | pd[i32x4_min_u];

assign
   pw8s  =          pd[i16x8_extadd_pairwise_i8x16_s],        // Pairwise signed add 8 bit
   pw16s =          pd[i32x4_extadd_pairwise_i16x8_s],        // Pairwise signed add 16 bit
   pw8   =  pw8s  | pd[i16x8_extadd_pairwise_i8x16_u],
   pw16  =  pw16s | pd[i32x4_extadd_pairwise_i16x8_u];

`ifdef   ENABLE_NEGABSSAT
assign
   enB   = ~pw8
         & ~pw16
         & ~( (pd[i8x16_abs])
            | (pd[i16x8_abs])
            | (pd[i32x4_abs])
            | (pd[i8x16_neg])
            | (pd[i16x8_neg])
            | (pd[i32x4_neg]) );
`else
assign
   enB   =  1'b1;
`endif //ENABLE_NEGABSSAT

assign
   mul   =     pd[i16x8_mul]
         |     pd[i32x4_mul]
   `ifdef   ENABLE_DOT_PRODUCT
         |     pd[i32x4_dot_i16x8_s]
   `endif
         |     pd[i32x4_extmul_dbl_i16x8_s]
         |     pd[i32x4_extmul_dbl_i16x8_u]
         |     pd[i16x8_extmul_dbl_i8x16_s]
         |     pd[i16x8_extmul_dbl_i8x16_u];

// Recode multiply functions
assign
   emulfn[0]  =  pd[i16x8_extmul_dbl_i8x16_s]
              |  pd[i16x8_extmul_dbl_i8x16_u],       //  8-bit multiplies

   emulfn[1]  =  pd[i16x8_mul]                       // 16-bit multiplies
   `ifdef   ENABLE_DOT_PRODUCT
              |  pd[i32x4_dot_i16x8_s]
   `endif //ENABLE_DOT_PRODUCT
              |  pd[i32x4_extmul_dbl_i16x8_s]
              |  pd[i32x4_extmul_dbl_i16x8_u],

   emulfn[2]  =  pd[i32x4_mul],

   emulfn[3]  =  pd[i16x8_extmul_dbl_i8x16_s]
   `ifdef   ENABLE_DOT_PRODUCT
              |  pd[i32x4_dot_i16x8_s]
   `endif //ENABLE_DOT_PRODUCT
              |  pd[i32x4_extmul_dbl_i16x8_s],       // signed multiplies
   emulfn[4]  =  pd[i8x16_swizzle]
              |  pd[i8x16_shuffle];                  // Permutes

// RECODE PIPELINE
always @(posedge clock) if (eSIMD && SIMDir) MULFN[4:0]  <= `tCQ  emulfn[4:0];
                   else if (done)            MULFN[4:0]  <= `tCQ  5'h00;
always @(posedge clock) if (eSIMD)           LDBW        <= `tCQ |emulfn[4:0];
                      else                   LDBW        <= `tCQ  1'b0;
always @(posedge clock) if (eSIMD && SIMDir) MUL         <= `tCQ  mul;


// RECODE PIPELINE
always @(posedge clock) if (eSIMD)              PW8      <= `tCQ  pw8;
always @(posedge clock) if (eSIMD)              PW8s     <= `tCQ  pw8s;
always @(posedge clock) if (eSIMD)              PW16     <= `tCQ  pw16;
always @(posedge clock) if (eSIMD)              PW16s    <= `tCQ  pw16s;
always @(posedge clock) if (eSIMD)              ENB      <= `tCQ  enB & ~mul;
always @(posedge clock) if (eSIMD)              SUBT     <= `tCQ  subt;

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//                      /////////////////////////////////////////////////////////////////////////////////////////////////////////
// FUNCTION DECODE      /////////////////////////////////////////////////////////////////////////////////////////////////////////
//                      /////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

// DECODE FUNCTION
wire  [102:0]   wasm_ctrl;
WASM_decode iWdec (
   .wasm_ctrl     (wasm_ctrl),
   .illegal_opcode(illegal_opcode_wasm),
   .FN            (FN[7:0]),
   .ENwasm        (ENwasm),
   .done          (done)
   );

wire  [7:0] Wctrl;
wire  [6:0] cc;
wire  [1:0] tSH;
wire  [1:0] zSH;
wire [11:0] sSL;
wire  [7:0] sYA;
wire  [3:0] sYB;
wire  [5:0] sXN;
wire  [5:0] sSPL;
wire  [3:0] sEL;
wire  [3:0] sNR;
wire  [4:0] sSI;
wire  [3:0] truth;

wire        any_LBSS,   any_logic,  any_shift,  any_splat,  bitselect,  enableSM,   enableV1,
            abs8,       abs16,      abs32,      eq8,        eq16,       eq32,
            lm8,        lm16,       lm32,       ln8,        ln16,       ln32,
            neg8,       neg16,      neg32,
            smax,       smin,       umax,       umin,
            ss8,        ss16,                   us8,        us16,
            output_scalar;

assign   {Wctrl[7:0],     cc[6:0],  tSH[1:0],  zSH[1:0],   sSL[11:0],   sYA[7:0],  sYB[3:0],
            sXN[5:0],   sSPL[5:0],  sEL[3:0],  sSI[4:0],   sNR[3:0],  truth[3:0],
            any_LBSS,   any_logic,  any_shift, any_splat, bitselect,  enableSM,  enableV1,
            abs8,       abs16,      abs32,
            eq8,        eq16,       eq32,
            lm8,        lm16,       lm32,
            ln8,        ln16,       ln32,
            neg8,       neg16,      neg32,
            smax,       smin,       umax,      umin,
            ss8,        ss16,       us8,       us16,  output_scalar}    =  wasm_ctrl[102:0];

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// MULTIPLY & PERMUTE
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//
// Multiplies:
// Simple multiplies that are modulo the operand size;  16x16 returns 16-bits and 32x32 returns 32-bits
// All other multiplications are done in two parts.  The High-speed serial multipliers generate a lower product and an
// upper redundant partial product (sum and carry).  These must be added together in the main SIMD adder and the result "Q"
// and the lower product "L" must then be re-arranged to form double length products.  As both high and low results are
// created in a single operation they must be returned to both V0 and V1.  One of these may be dropped if not needed.
// So the adder results and the lower product are presented here for rearrangement.
//
// These are bit-serial Baugh-Wooley multipliers that obviate the need for sign-extension (a la Agrawal and Rao).  They use
// redundant adders for speed.  The multipliers takes 8, 16 or 32 fast clock cycles to multiply followed by a finish slow cycle
// that performs the upper add of the redundant sum and carry and then does a double 'extend' to put things in the right place.
//
// The dot-product requires a second add.  So as the extmul operators return all 16 products we just need to do a final
// 16-bit add to complete, except the ordering is different so we have a dotmul instead
// So "dot" does not exist and the transsembler does the following 'dotmul' followed by an 'i32x4_add'
//
// Permutes:
// The permute "swoozle" uses the high-speed clock to do a byte select at the high clock rate.  The select bytes are loaded into
// a high-speed V0 clone and then a 32-byte to 1 multiplexor selected by the lower 5 bits of the lowest V0 byte.  The selected
// byte is then shifted into the MS end of the V0 clone as each select byte is shifted out. So the result ends up in the V0 clone.
// We replace these with a "Swoozle" instruction that works in the fast clock domain.  V1 and V2 are held static and a V0
// clone is byte shifted per cycle.  The output byte selects a byte from V0 or V1 and places it in the input byte of the
// V0 clone.  16 fast clocks later we come back to normal clock and pop V2 and V1 leaving the resullt  in V0
// A WASM swizzle is a swoozle with V2  loaded with zero.  Transsembler time again!!
// <<<< WE USE THE SWIZZLE OPCODE FOR THE SWOOZLE.  CAVEAT EMPTOR >>>>>
// A shuffle is a swoozle with the immediate loaded to V0 first
//

/* <<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>> *
 * <<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<< HIGH CLOCK RATE SECTION >>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>> *
 * <<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>> */

// (* SiArt : Multi-cycle path V1, V2 to swoozle.   Fast path L[4:0] to swoozle *)
wire  [127:0]  L, H, C;
wire    [7:0]  swoozle  = {V2[127:0], V1[127:0]} >> (8 * L[4:0]);    // Very high speed!!! -- clone of V0
wire   [31:0]  BS       = {swoozle,L[103:096],L[71:64],L[39:32]};    // Very high speed!!! -- shift data between blocks
wire    [3:0]  fini;

for (g=0; g<4; g=g+1) begin : MPR
   A2_BWmultiply_BSW  Mult (        //                BSW:  Byte\Short\Word
      .LPo        ( L[32*g+:32]),   //                Lower Product: Integer result of multiply and result of permute
      .UPPo       ( H[32*g+:32]),   //                Upper Partial Product
      .UPCo       ( C[32*g+:32]),   //                Upper Partial Carry
      .iMC        (V1[32*g+:32]),   //                Multiplicand Operand to be multiplied
      .iMP        (V0[32*g+:32]),   //                Multiplier   Operand to multiply by
      .iBS        (BS[ 8*g+:8 ]),   //                Byte shift links
      .done       (fini[g]),        //                Output is valid
      .ldBW       (LDBW & run),     //  (* SiART *)   load BW multiplier
      .fn         (MULFN[4:0]),     //                signed\unsigned  bit-2: 32-bit, bit-1: 16-bit, bit-0: 8-bit -- one-hot
      .setup      (CF[23:0]),       //                adjustable configuration for high-speed operation
      .fast_clock (fast_clock),
      .slow_clock (clock),
      .reset      (reset)
      );
   end
                                    //  (* SiART *)  'ldBW' signal transitions between the slow_clock and fast_clock through a
                                    //               multistage synchronizer.  All others set-up before or same as ldBW.
                                    //               'done' signal set-up a whole slow clock cycle
assign   done  = &fini[3:0];
/* <<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>> *
 * <<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<< END HIGH CLOCK RATE SECTION >>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>> *
 * <<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>> */

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//                                                                         //////////////////////////////////////////////////////
// DATA PATH 1                                                             //////////////////////////////////////////////////////
// LOGIC, SHIFT, EXTRACT, BITMASK, EXTEND, NARROW, REDUCE, SPLAT|REPLACE   //////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

// BITWISE LOGIC ----------------------------------------------------------------------------------------------------------------
wire  [127:0]  bitwise;
wire  [ 15:0]  byteOR;
wire  [  7:0]  shortOR;
wire  [  3:0]  wordOR;
reg   [  6:0]  index;

for (g=0; g<128; g=g+1) begin : Logic
   assign   bitwise[g] = truth >> {V1[g],V0[g]};
   end

// A bit-wise select: a bit from V1 or V2 with V0 as the select,  Uses the Andnot from above
wire  [127:0]  LFN   = bitwise[127:0] | V0[127:0] & V2[127:0] & {128{bitselect}};

// Feeds into the Shifter
// SHIFTER ----------------------------------------------------------------------------------------------------------------------
always @* (* parallel_case *) case(1'b1)
   sSI[0]   : index[6:0] = {IMM[3:0],  3'b000};
   sSI[1]   : index[6:0] = {IMM[3:0],  3'b000};
   sSI[2]   : index[6:0] = {IMM[2:0], 4'b0000};
   sSI[3]   : index[6:0] = {IMM[2:0], 4'b0000};
   sSI[4]   : index[6:0] = {IMM[1:0],5'b00000};
   default    index[6:0] =         7'b0000000;
   endcase

wire  [  4:0]  distance  = zSH != 3 ? iS[4:0] : index[4:0];  // index[6:5] used to select 1 of 4 shifters! see 'lane' below

// Instantiate 4 x  A2_shifter_BSW
for (g=0; g<4; g=g+1) begin : SHIFT
   A2_shifter_BSW  Shift (
      .SHo  (SHF[32*g+:32]),  // Shift Data Out
      .iSH  (LFN[32*g+:32]),  // Shift Data In
      .dSH  (distance[4:0]),  // Shift distance
      .zSH  (zSH),            // Shift size
      .tSH  (tSH)             // Shift type
      );
   end

// Scalar Splat & Constant input ------------------------------------------------------------------------------------------------
wire  [127:0]  SPL;
A2_mux #(6,128) i_SPL  (.z(SPL), .s(sSPL[5:0]),.d({                 SHF[127:00],       // Bypass
                                                          iS[31:0], SHF[127:32],       // Shift-in Scalar Constant
                                                    96'h0,iS[31:0],                    // Load 32 zero extended
                                                   {    4{iS[31:0]}},                  // Splat Words
                                                   {    8{iS[15:0]}},                  // Splat Shorts
                                                   {   16{iS[07:0]}}            }));   // Splat Bytes

// Narrow Lanes -----------------------------------------------------------------------------------------------------------------
reg   [127:0]  NRW;
`ifdef   ENABLE_NARROW
always @* (* parallel_case *) case(1'b1)
   sNR[0] : `BYTES  NRW[ 8*i+: 8] =  i< 8 ? `US08(   SPL[16*i+:16])  : `US08(   V1[16*(i-8)+:16] ); end // i8x16_narrow_i16x8_u
   sNR[1] : `BYTES  NRW[ 8*i+: 8] =  i< 8 ? `SS08(`S(SPL[16*i+:16])) : `SS08(`S(V1[16*(i-8)+:16])); end // i8x16_narrow_i16x8_s
   sNR[2] : `SHORTS NRW[16*i+:16] =  i< 4 ? `US16(   SPL[32*i+:32])  : `US16(   V1[32*(i-4)+:32] ); end // i16x8_narrow_i32x4_u
   sNR[3] : `SHORTS NRW[16*i+:16] =  i< 4 ? `SS16(`S(SPL[32*i+:32])) : `SS16(`S(V1[32*(i-4)+:32])); end // i16x8_narrow_i32x4_s
   default                            NRW[127:0]    =  SPL[127:0];
   endcase
`else
always @* NRW[127:0] = SPL[127:0];
`endif   //ENABLE_NARROW

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//                         //////////////////////////////////////////////////////////////////////////////////////////////////////
//   SCALAR DATA PATH      //////////////////////////////////////////////////////////////////////////////////////////////////////
//                         //////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

// Zero detect on LFN on a per byte basis
for (g=0;g<16;g=g+1) begin : bOR
   assign byteOR[g]  = |LFN[8*g+:8];
   end

for (g=0;g<8;g=g+1) begin : sOR
   assign shortOR[g] = byteOR[2*g+1]  | byteOR[2*g];
   end

for (g=0;g<4;g=g+1) begin : wOR
   assign wordOR[g]  = shortOR[2*g+1] | shortOR[2*g];
   end

wire
   all_true8  = &byteOR[15:0],
   all_true16 = &shortOR[7:0],
   all_true32 = &wordOR[3:0],
   any_true   = |wordOR[3:0];

// Bitmasks ---------------------------------------------------------------------------------------------------------------------
wire  [15:0]   bm08  = {LFN[127],LFN[119],LFN[111],LFN[103],LFN[095],LFN[087],LFN[079],LFN[071],   // Bitmasks
                        LFN[063],LFN[055],LFN[047],LFN[039],LFN[031],LFN[023],LFN[015],LFN[007]};
wire  [ 7:0]   bm16  = {LFN[127],LFN[111],LFN[095],LFN[079],LFN[063],LFN[047],LFN[031],LFN[015]};
wire  [ 7:0]   bm32  = {4'h0,    LFN[127],LFN[095],LFN[063],LFN[031]};

// Mulitplex Scalar Out --------------------------------------------------------------------------------------------------------
wire  [31:0]   lane;
wire  [ 1:0]   TF       =  sSL[11]  ? ( all_true8  ? 2'b10 : 2'b01)
                        :  sSL[10]  ? ( all_true16 ? 2'b10 : 2'b01)
                        :  sSL[09]  ? ( all_true32 ? 2'b10 : 2'b01)
                        :  sSL[08]  ? ( any_true   ? 2'b10 : 2'b01) : 2'b00;
wire           en_lane  = |sSL[4:0];
// wire           gv_WASMo =  ENwasm & (|sSL[7:0] | TF[1] | TF[0]);
// Extract lanes from each shifter
wire  [3:0] sLane = en_lane << index[6:5];
A2_mux #( 4,32) i_Ln (.z(lane[31:0]), .s(sLane[3:0]), .d(SHF[127:0]));

A2_mux #(10,32) i_So (.z(Sw[31:0]),
                      .s({TF[1:0],sSL[7:0]}),
                      .d({ 32'hffffffff,                    // True
                           32'h00000000,                    // False
                           28'h0000000,   bm32[03:0],       // bitmask 32
                           24'h000000,    bm16[07:0],       // bitmask 16
                           16'h0000,      bm08[15:0],       // bitmask  8
                           24'h000000,    lane[07:0],       // extract  8-bit lane unsigned
                           16'h0000,      lane[15:0],       // extract 16-bit lane unsigned
                           {24{lane[7]}}, lane[07:0],       // extract  8-bit lane signed
                           {16{lane[15]}},lane[15:0],       // extract 16-bit lane signed
                                          lane[31:0] }));   // extract 32-bit lane

// Load Lane Mask ---------------------------------------------------------------------------------------------------------------
// This works by splatting iS across the word and then only selecting which bytes to write to, with this:
wire   [15:0]
   eBLane   =  1'b1    << {IMM[3:0]       },
   eSLane   =  2'b11   << {IMM[2:0], 1'b0 },
   eWLane   =  4'b1111 << {IMM[1:0], 2'b00};

wire   [15:0] eLane;
A2_mux #(4,16) iEL (.z(eLane[15:0]), .s(sEL[3:0]), .d({eBLane,eSLane,eWLane,16'hffff}));

for (g=0; g<16; g=g+1) begin : xmsk
   assign mask[8*g+:8] = {8{eLane[g] | ENsimd}};
   end

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// ADD / SUBTRACT,  RELATIONAL, EXTEND, EXTEND MULTIPLIES
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Adder / Subtractors Inputs ---------------------------------------------------------------------------------------------------
// Good 'ole Agrawal and Rao technique for addition of signed numbers used here to reduce the sign-extension burden.
// We still have an FO = 80 for PW8S and FO = 72 for PW16S, but they can be retimed!  The fanout on the sign bits is much lower.
// PAIRWISE Operations are emulated here:
// We do a shift of the input operand then add the two together as a "add_even_extend" operation

wire  [127:0] pwbl   = {{ 8{PW8s}},   PW8s ^ V1[119], V1[118:112],      // If signed, invert the sign bit and add a correction
                        { 8{PW8s}},   PW8s ^ V1[103], V1[102:096],      // factor of -1 * 2^8
                        { 8{PW8s}},   PW8s ^ V1[087], V1[086:080],
                        { 8{PW8s}},   PW8s ^ V1[071], V1[070:064],      // Pairwise add this way has no re-arrangement
                        { 8{PW8s}},   PW8s ^ V1[055], V1[054:048],      // requirement after the adders
                        { 8{PW8s}},   PW8s ^ V1[039], V1[038:032],
                        { 8{PW8s}},   PW8s ^ V1[023], V1[022:016],      // For unsigned PW8S is low so upper bits are all zero
                        { 8{PW8s}},   PW8s ^ V1[007], V1[006:000] };    // and lower msb is NOT inverted!

wire  [127:0] pwwl   = {{16{PW16s}}, PW16s ^ V1[111], V1[110:096],
                        {16{PW16s}}, PW16s ^ V1[079], V1[078:064],
                        {16{PW16s}}, PW16s ^ V1[047], V1[046:032],
                        {16{PW16s}}, PW16s ^ V1[015], V1[014:000] };

wire  [127:0] pwbr   = {  8'h00,      PW8s ^ V0[119], V0[118:112],      // Correction factor not used on this side so the
                          8'h00,      PW8s ^ V0[103], V0[102:096],      // upper is all zeros.  MSB of lower is still inverted
                          8'h00,      PW8s ^ V0[087], V0[086:080],      // for signed operation
                          8'h00,      PW8s ^ V0[071], V0[070:064],
                          8'h00,      PW8s ^ V0[055], V0[054:048],
                          8'h00,      PW8s ^ V0[039], V0[038:032],
                          8'h00,      PW8s ^ V0[023], V0[022:016],
                          8'h00,      PW8s ^ V0[007], V0[006:000] };

wire  [127:0] pwwr   = { 16'h0000,   PW16s ^ V0[111], V0[110:096],
                         16'h0000,   PW16s ^ V0[079], V0[078:064],
                         16'h0000,   PW16s ^ V0[047], V0[046:032],
                         16'h0000,   PW16s ^ V0[015], V0[014:000] };

// wire  [ 15:0] ENA    =  16'hffff;
// wire  [127:0] ena    = { {8{ENA[15]}}, {8{ENA[14]}}, {8{ENA[13]}}, {8{ENA[12]}},
//                          {8{ENA[11]}}, {8{ENA[10]}}, {8{ENA[09]}}, {8{ENA[08]}},
//                          {8{ENA[07]}}, {8{ENA[06]}}, {8{ENA[05]}}, {8{ENA[04]}},
//                          {8{ENA[03]}}, {8{ENA[02]}}, {8{ENA[01]}}, {8{ENA[00]}}};
// wire  [127:0] subop  =   ena[127:0] & (V0[127:0] ^ {128{SUBT}});           // Invert for subtract ops ENA for partial products

wire  [127:0] left, right;

wire  selelse  =  ~MUL & ~PW8 & ~PW16;
A2_mux #(4,128) iLEFT  (.z(left), .s({MUL,PW8,PW16,ENB}),     .d({H[127:0],pwbl[127:0],pwwl[127:0],V1[127:0]}));
A2_mux #(4,128) iRIGHT (.z(right),.s({MUL,PW8,PW16,selelse}), .d({C[127:0],pwbr[127:0],pwwr[127:0],V0[127:0]^{128{SUBT}}}));

wire  [143:0]  lhs   =  {1'b0,  left[127:120], cc[5],  left[119:112], cc[3],  left[111:104], cc[1],  left[103:096],
                         1'b0,  left[095:088], cc[5],  left[087:080], cc[3],  left[079:072], cc[1],  left[071:064],
                         1'b0,  left[063:056], cc[5],  left[055:048], cc[3],  left[047:040], cc[1],  left[039:032],
                         1'b0,  left[031:024], cc[5],  left[023:016], cc[3],  left[015:008], cc[1],  left[007:000]};

wire  [143:0]  rhs   =  {1'b0, right[127:120], cc[6], right[119:112], cc[4], right[111:104], cc[2], right[103:096],
                         1'b0, right[095:088], cc[6], right[087:080], cc[4], right[079:072], cc[2], right[071:064],
                         1'b0, right[063:056], cc[6], right[055:048], cc[4], right[047:040], cc[2], right[039:032],
                         1'b0, right[031:024], cc[6], right[023:016], cc[4], right[015:008], cc[2], right[007:000]};

// ADDERS------------------------------------------------------------------------------------------------------------------------
wire     [143:0]  ks;
assign ks[143:108]   =   lhs[143:108] + rhs[143:108] + cc[0],
       ks[107:072]   =   lhs[107:072] + rhs[107:072] + cc[0],
       ks[071:036]   =   lhs[071:036] + rhs[071:036] + cc[0],
       ks[035:000]   =   lhs[035:000] + rhs[035:000] + cc[0];

// Sums -------------------------------------------------------------------------------------------------------------------------
wire  [127:0]  sum   =  {ks[142:135],ks[133:126],ks[124:117],ks[115:108],
                         ks[106:099],ks[097:090],ks[088:081],ks[079:072],
                         ks[070:063],ks[061:054],ks[052:045],ks[043:036],
                         ks[034:027],ks[025:018],ks[016:009],ks[007:000]};

// Flags ------------------------------------------------------------------------------------------------------------------------
// Carry flags
wire  [ 15:0]  c08   =  {ks[143],ks[134],ks[125],ks[116],ks[107],ks[098],ks[089],ks[080],
                         ks[071],ks[062],ks[053],ks[044],ks[035],ks[026],ks[017],ks[008]};

wire  [  7:0]  c16   =  {c08[15],c08[13],c08[11],c08[09],c08[07],c08[05],c08[03],c08[01]};
wire  [  3:0]  c32   =  {c16[07],c16[05],c16[03],c16[01]};
// Zero flags
wire  [ 15:0]  z08   =  {~|sum[127:120],~|sum[119:112],~|sum[111:104],~|sum[103:096],
                         ~|sum[095:088],~|sum[087:080],~|sum[079:072],~|sum[071:064],
                         ~|sum[063:056],~|sum[055:048],~|sum[047:040],~|sum[039:032],
                         ~|sum[031:024],~|sum[023:016],~|sum[015:008],~|sum[007:000]};

wire  [  7:0]  z16   =  {z08[15] & z08[14], z08[13] & z08[12], z08[11] & z08[10], z08[09] & z08[08],
                         z08[07] & z08[06], z08[05] & z08[04], z08[03] & z08[02], z08[01] & z08[00]};
wire  [  3:0]  z32   =  {z16[07] & z16[06], z16[05] & z16[04], z16[03] & z16[02], z16[01] & z16[00]};

// Sign (negative) flags
wire  [ 15:0]  n08   =  {sum[127],sum[119],sum[111],sum[103],sum[095],sum[087],sum[079],sum[071],
                         sum[063],sum[055],sum[047],sum[039],sum[031],sum[023],sum[015],sum[007]};
wire  [  7:0]  n16   =  { n08[15], n08[13], n08[11], n08[09], n08[07], n08[05], n08[03], n08[01]};
wire  [  3:0]  n32   =  { n16[07], n16[05], n16[03], n16[01]};

// Overflow flags
wire  [ 15:0]  s08l  =  { left[127], left[119], left[111], left[103], left[095], left[087], left[079], left[071],
                          left[063], left[055], left[047], left[039], left[031], left[023], left[015], left[007]};
wire  [ 15:0]  s08r  =  {right[127],right[119],right[111],right[103],right[095],right[087],right[079],right[071],
                         right[063],right[055],right[047],right[039],right[031],right[023],right[015],right[007]};

wire  [ 15:0]  v08   =   ~s08l[15:0] & ~s08r[15:0] & n08[15:0] | s08l[15:0] & s08r[15:0] & ~n08[15:0];
wire  [  7:0]  v16   =  { v08[15],v08[13],v08[11],v08[9],v08[7],v08[5],v08[3],v08[1] };
wire  [  3:0]  v32   =  { v16[07],v16[05],v16[03],v16[01] };

// Collate Flags ----------------------------------------------------------------------------------------------------------------
// NOTA BENE:  We abondoned all other relational operators to save space.  They can all be derived from these three anyway!!
wire  [15:0]   eq, lt, lo, mxmn;
`ifdef   ENABLE_NEGABSSAT
wire  [15:0]   neg, abs, ssat, usat, sinv;
`endif //ENABLE_NEGABSSAT

for (g=0;g<4;g=g+1) begin : RO
   assign
      // collate relational flags
      eq[4*g+3]   =  eq8  &   z08[4*g+3]                 | eq16 &  z16[2*g+1]               | eq32 &  z32[g],
      eq[4*g+2]   =  eq8  &   z08[4*g+2]                 | eq16 &  z16[2*g+1]               | eq32 &  z32[g],
      eq[4*g+1]   =  eq8  &   z08[4*g+1]                 | eq16 &  z16[2*g+0]               | eq32 &  z32[g],
      eq[4*g+0]   =  eq8  &   z08[4*g+0]                 | eq16 &  z16[2*g+0]               | eq32 &  z32[g],

      lt[4*g+3]   =  lm8  &  (n08[4*g+3] ^ v08[4*g+3])   | lm16 & (n16[2*g+1] ^ v16[2*g+1]) | lm32 & (n32[g] ^ v32[g]),
      lt[4*g+2]   =  lm8  &  (n08[4*g+2] ^ v08[4*g+2])   | lm16 & (n16[2*g+1] ^ v16[2*g+1]) | lm32 & (n32[g] ^ v32[g]),
      lt[4*g+1]   =  lm8  &  (n08[4*g+1] ^ v08[4*g+1])   | lm16 & (n16[2*g+0] ^ v16[2*g+0]) | lm32 & (n32[g] ^ v32[g]),
      lt[4*g+0]   =  lm8  &  (n08[4*g+0] ^ v08[4*g+0])   | lm16 & (n16[2*g+0] ^ v16[2*g+0]) | lm32 & (n32[g] ^ v32[g]),

      lo[4*g+3]   =  ln8  &  ~c08[4*g+3]                 | ln16 & ~c16[2*g+1]               | ln32 & ~c32[g],
      lo[4*g+2]   =  ln8  &  ~c08[4*g+2]                 | ln16 & ~c16[2*g+1]               | ln32 & ~c32[g],
      lo[4*g+1]   =  ln8  &  ~c08[4*g+1]                 | ln16 & ~c16[2*g+0]               | ln32 & ~c32[g],
      lo[4*g+0]   =  ln8  &  ~c08[4*g+0]                 | ln16 & ~c16[2*g+0]               | ln32 & ~c32[g],
      // collate min/max flags
      mxmn[4*g+3] =  smax &   lt[4*g+3]   | smin & ~lt[4*g+3]  | umax & lo[4*g+3] | umin & ~lo[4*g+3],
      mxmn[4*g+2] =  smax &   lt[4*g+2]   | smin & ~lt[4*g+2]  | umax & lo[4*g+2] | umin & ~lo[4*g+2],
      mxmn[4*g+1] =  smax &   lt[4*g+1]   | smin & ~lt[4*g+1]  | umax & lo[4*g+1] | umin & ~lo[4*g+1],
      mxmn[4*g+0] =  smax &   lt[4*g+0]   | smin & ~lt[4*g+0]  | umax & lo[4*g+0] | umin & ~lo[4*g+0];

`ifdef   ENABLE_NEGABSSAT
   assign
      // collate negate, absolute and saturate flags
      neg[4*g+3]  =  neg8 &  ~byteOR[4*g+3] | neg16 & ~shortOR[2*g+1] | neg32 & ~wordOR[3],
      neg[4*g+2]  =  neg8 &  ~byteOR[4*g+2] | neg16 & ~shortOR[2*g+1] | neg32 & ~wordOR[2],
      neg[4*g+1]  =  neg8 &  ~byteOR[4*g+1] | neg16 & ~shortOR[2*g+0] | neg32 & ~wordOR[1],
      neg[4*g+0]  =  neg8 &  ~byteOR[4*g+0] | neg16 & ~shortOR[2*g+0] | neg32 & ~wordOR[0],

      abs[4*g+3]  =  abs8 &   V0[32*g+31]   | abs16 & V0[32*g+31]     | abs32 & V0[127],
      abs[4*g+2]  =  abs8 &   V0[32*g+23]   | abs16 & V0[32*g+31]     | abs32 & V0[ 95],
      abs[4*g+1]  =  abs8 &   V0[32*g+15]   | abs16 & V0[32*g+15]     | abs32 & V0[ 63],
      abs[4*g+0]  =  abs8 &   V0[32*g+07]   | abs16 & V0[32*g+15]     | abs32 & V0[ 31],

      ssat[4*g+3] =  ss8  &   v08[4*g+3]    | ss16  &  v16[2*g+1],                           // detect SIGNED overflow
      ssat[4*g+2] =  ss8  &   v08[4*g+2]    | ss16  &  v16[2*g+1],
      ssat[4*g+1] =  ss8  &   v08[4*g+1]    | ss16  &  v16[2*g+0],
      ssat[4*g+0] =  ss8  &   v08[4*g+0]    | ss16  &  v16[2*g+0],

      sinv[4*g+3] =  ss8  &   n08[4*g+3]    | ss16  &  n16[2*g+1],                           // detect SIGN of overflow
      sinv[4*g+2] =  ss8  &   n08[4*g+2]    | ss16  &  n16[2*g+1],
      sinv[4*g+1] =  ss8  &   n08[4*g+1]    | ss16  &  n16[2*g+0],
      sinv[4*g+0] =  ss8  &   n08[4*g+0]    | ss16  &  n16[2*g+0],

      usat[4*g+3] =  us8  &  (SUBT ^ c08[4*g+3]) | us16  & (SUBT ^ c16[2*g+1]),              // detect UNSIGNED overflow
      usat[4*g+2] =  us8  &  (SUBT ^ c08[4*g+2]) | us16  & (SUBT ^ c16[2*g+1]),              // invert carry to borrow
      usat[4*g+1] =  us8  &  (SUBT ^ c08[4*g+1]) | us16  & (SUBT ^ c16[2*g+0]),              //    for subtraction
      usat[4*g+0] =  us8  &  (SUBT ^ c08[4*g+0]) | us16  & (SUBT ^ c16[2*g+0]);
`endif //ENABLE_NEGABSSAT
   end

// The relational operators are mutually exclusive so we can just OR them!
wire  [15:0] ro   = eq[15:0] | lt[15:0] | lo[15:0];

// Arithmetic ===================================================================================================================
reg   [127:0]  TRUEFALSE, MINMAX, AVG8, AVG16, POPCNT;
wire  [127:0]  YA;

// POPulation Count ----------------------------
always @*
   if (FN[7:0] == i8x16_popcnt) begin
      POPCNT =  128'h0;
      for (i=0; i<16; i=i+1) begin
         for (j=0; j<8; j=j+1) begin
            POPCNT[8*i+:8] =  POPCNT[8*i+:8] + V0[8*i+j];
            end
         end
      end
   else POPCNT = 128'h0;

// Average, Min/Max and Relationals ------------
always @* begin
   for(i=0; i<16; i=i+1)  begin
      TRUEFALSE[ 8*i+: 8]  =  ro[i]    ?    8'hff       : 8'h00;
      MINMAX   [ 8*i+: 8]  =  mxmn[i]  ?  V0[ 8*i+: 8]  : V1[ 8*i+: 8];
      AVG8     [ 8*i+: 8]  = {c08[i],sum[ 8*i+1+: 7]};
      end
   for(i=0; i<8;  i=i+1)  AVG16[16*i+:16] = {c16[i],sum[16*i+1+:15]};
   end

A2_mux #(8,128) i_YA  (.z(YA[127:0]), .d({TRUEFALSE,MINMAX,AVG8, AVG16, sum, POPCNT, L, V0}), .s(sYA[7:0]));

// NEGATE, ABSOLUTE, SATURATE U/S ===============================================================================================

`ifdef   ENABLE_NEGABSSAT
reg   [127:0]  YB;
always @* case (1'b1)
   sYB[0]   : `BYTES  YB[ 8*i+: 8] =  neg[i]  ?  8'b0                                           : sum[ 8*i+: 8]; end
   sYB[1]   : `BYTES  YB[ 8*i+: 8] = !abs[i]  ?  V0[ 8*i+: 8]                                   : sum[ 8*i+: 8]; end
   sYB[2]   : `BYTES  YB[ 8*i+: 8] =  usat[i] ? {8{~SUBT}}                                      : sum[ 8*i+: 8]; end
   sYB[3]   : `BYTES  YB[ 8*i+: 8] =  ss16 && (i%2 == 0) && ssat[i] ? {8{sinv[i]}}
                                   :                        ssat[i] ? {~sinv[i],{7{sinv[i]}}}   : sum[ 8*i+: 8]; end
   default            YB[127:0]    =  128'h0;
   endcase
`else
reg   [127:0]  YB = 128'h0;
`endif //ENABLE_NEGABSSAT

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// RESULT COLLATION        //////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//  Unsigned extends simply fill the upper bits with zero
//  Signed extends fill the upper bits with the sign of the lower bits replicated across all the upper bits
//  This can be acheived by testing for less than zero which returns all ones if true else all zeros
//  A support instruction can help that tests V0 against zero and pushes the result into V1 and keeps V0.  Otherwise we need:
//  "dup 0 < swap extend" and WASM has no 'dup' or 'swap' so: " a 0 < a extend "
//

wire  [127:0]  lower = YA[127:0]  | YB[127:0];
wire  [127:0]  upper;

A2_mux #(3,128) iUPPER (.z(upper), .s({enableV1,enableSM,any_LBSS}), .d({V1[127:0],sum[127:0],NRW[127:0]}));

// Byte Permutations -----------------------------------------------------------------------------------------------------------
wire  [7:0] a [0:15];
wire  [7:0] b [0:15];

for (g=0; g<16; g=g+1) begin : ab
   assign
      a[g][7:0]   =  upper[8*g+:8],
      b[g][7:0]   =  lower[8*g+:8];
   end

wire  [127:0]
   // Pass
   ZPASS =  { a[15], a[14], a[13], a[12], a[11], a[10], a[09], a[08], a[07], a[06], a[05], a[04], a[03], a[02], a[01], a[00]},
   YPASS =  { b[15], b[14], b[13], b[12], b[11], b[10], b[09], b[08], b[07], b[06], b[05], b[04], b[03], b[02], b[01], b[00]},
   // Extend 8 to 16-bit --  get upper bits from a
   ZDX08 =  { a[15], b[15], a[14], b[14], a[13], b[13], a[12], b[12], a[11], b[11], a[10], b[10], a[09], b[09], a[08], b[08]},
   YDX08 =  { a[07], b[07], a[06], b[06], a[05], b[05], a[04], b[04], a[03], b[03], a[02], b[02], a[01], b[01], a[00], b[00]},
   // Extend 16 to 32-bit -- get upper bits from a
   ZDX16 =  { a[15], a[14], b[15], b[14], a[13], a[12], b[13], b[12], a[11], a[10], b[11], b[10], a[09], a[08], b[09], b[08]},
   YDX16 =  { a[07], a[06], b[07], b[06], a[05], a[04], b[05], b[04], a[03], a[02], b[03], b[02], a[01], a[00], b[01], b[00]},
   // Narrow 16 to 8-bit --  put upper bits into a
   ZDN08 =  { a[15], a[13], a[11], a[09], a[07], a[05], a[03], a[01], b[15], b[13], b[11], b[09], b[07], b[05], b[03], b[01]},
   YDN08 =  { a[14], a[12], a[10], a[08], a[06], a[04], a[02], a[00], b[14], b[12], b[10], b[08], b[06], b[04], b[02], b[00]},
   // Narrow 32 to 16-bit -- put upper bits into a
   ZDN16 =  { a[15], a[14], a[11], a[10], a[07], a[06], a[03], a[02], b[15], b[14], b[11], b[10], b[07], b[06], b[03], b[02]},
   YDN16 =  { a[13], a[12], a[09], a[08], a[05], a[04], a[01], a[00], b[13], b[12], b[09], b[08], b[05], b[04], b[01], b[00]};
   // Mux it all together

wire  [127:0]  ZW, YW;

A2_mux #(6,128) i_ZW  (.z(ZW[127:0]), .d({ZDN16, ZDN08, ZDX16, ZDX08, YPASS, ZPASS}), .s(sXN[5:0]));
A2_mux #(6,128) i_YW  (.z(YW[127:0]), .d({YDN16, YDN08, YDX16, YDX08, ZPASS, YPASS}), .s(sXN[5:0]));

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//                                     //////////////////////////////////////////////////////////////////////////////////////////
//  Custom SIMD and Vector Registers   //////////////////////////////////////////////////////////////////////////////////////////
//                                     //////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
wire  [127:0]  YS,ZS;
wire  [  7:0]  Cctrl;
wire           count, clear, xnorscalar;
//wire           gv_simdo =  ENsimd & (FN[7:0] == simd_store_scalar);

A2_custom_simd iCSALU (
   // Outputs
   .YS            (YS[127:0]),
   .ZS            (ZS[127:0]),
   .So            (Sc[ 31:0]),
   .Cctrl         (Cctrl[7:0]),
   .loadEN        (loadEN),
   .setEN         ( setEN),
   .writeEN       (writeEN),
   .flipEN        (flipEN),
   .count         (count),
   .clear         (clear),
   .xnorscalar    (xnorscalar),
   .enEN          (enEN),
   .illegal_opcode(illegal_opcode_simd),
   // Inputs
   .V0            (V0[127:0]),
   .V1            (V1[127:0]),
   .V2            (V2[127:0]),
   .V3            (V3[127:0]),
   .EN            (EN[127:0]),
   .MR            (MR[127:0]),
   .iS            (iS[ 31:0]),
   .FN            (FN[  7:0]),
   .enable        (ENsimd)
   );

VectorRegs iVR (
   .V0            (V0[127:0]),
   .V1            (V1[127:0]),
   .V2            (V2[127:0]),
   .V3            (V3[127:0]),
   .EN            (EN[127:0]),
   .Mctrl        (Mctrl[1:0]),
   .YW            (YW[127:0]),
   .ZW            (ZW[127:0]),
   .YS            (YS[127:0]),
   .ZS            (ZS[127:0]),
   .MR            (MR[127:0]),
   .eLane         (eLane[15:0]),
   .Wctrl         (Wctrl[7:0]),
   .Cctrl         (Cctrl[7:0]),
   .iS_0          (iS[0]),
   .ENwasm        (ENwasm),
   .ENsimd        (ENsimd),
   .setEN         (setEN),
   .loadEN        (loadEN),
   .writeEN       (writeEN),
   .flipEN        (flipEN),
   .mrak          (mrak),
   .count         (count),
   .clear         (clear),
   .xnorscalar    (xnorscalar),
   .run           (run),
   .reset         (reset),
   .clock         (clock)
   );

endmodule

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

module   VectorRegs (
   output reg  [127:0]  V0, V1, V2, V3, EN,          // Vector Stack V0..V3, Memory Write Data & Memory bit mask ENable
   output wire [  1:0]  Mctrl,
   input  wire [127:0]  YW, ZW, YS, ZS, MR,
   input  wire [ 15:0]  eLane,
   input  wire [  7:0]  Wctrl, Cctrl,
   input  wire          iS_0,
   input  wire          ENwasm, ENsimd,
   input  wire          setEN,loadEN,writeEN,flipEN,
   input  wire          mrak,                         // Memory Read AcKnowledge
   input  wire          count,
   input  wire          clear,
   input  wire          xnorscalar,
   input  wire          run,
   input  wire          reset,
   input  wire          clock
   );

integer  i;

// CONTROL_PARAMETERS -----------------------------------------------------------------------------------------------------------
localparam  [3:0]    hold = 4'd0,    push  = 4'd1,    pop  = 4'd2,    swap = 4'd3,
                     urt4 = 4'd4,    nip   = 4'd5,    rot3 = 4'd6,    rot4 = 4'd7,
                     exe1 = 4'd8,    exe2  = 4'd9,    exe3 = 4'd10,   exe4 = 4'd11,
                     exe5 = 4'd12,   exe6  = 4'd13,   exe7 = 4'd14,   fill = 4'd15;
localparam  [3:0]    lbV0 = 4'b1010, lbV1  = 4'b1100, lnot = 4'b0101,
                     land = 4'b1000, lxor  = 4'b0110, lior = 4'b1110, lann = 4'b0100;
localparam  [1:0]    hldS = 2'd0,    pushS = 2'd1,    popS = 2'd2;
localparam  [1:0]    no   = 2'd0,    rd    = 2'd1,    wr   = 2'd2;
localparam  [1:0]    Byte = 0,       Shrt  = 1,       Word = 2;
localparam  [1:0]    Right= 0,       Arith = 1,       Left = 2;
//-------------------------------------------------------------------------------------------------------------------------------

wire    [3:0]  Vctrl;                                          // Vector Control
wire    [1:0]  Sctrl;                                          // Scalar Control,  Memory Control
assign
   {Vctrl, Sctrl, Mctrl} = ENwasm   ?  Wctrl
                         : ENsimd   ?  Cctrl
                         :            {hold,hldS,no};

reg    [31:0]  caseM;
reg     [7:0]  selV;                                           //  Vector Register Input selects
reg     [3:0]  ldV;                                            //  Vector Register Load enables
always @* if (clear)
              begin {ldV[3:0], selV[7:0]} = {4'b1111, 8'b00_00_00_00}; caseM = "clr ";  end //  1 Clear the stack
   else (* parallel_case *) case (Vctrl)
      push:   begin {ldV[3:0], selV[7:0]} = {4'b1111, 8'b11_10_01_00}; caseM = "push";  end //  1 Push to stack
      pop :   begin {ldV[3:0], selV[7:0]} = {4'b0111, 8'b00_11_10_01}; caseM = "pop ";  end //  2 Remove from stack (drop)
      swap:   begin {ldV[3:0], selV[7:0]} = {4'b0011, 8'bxx_xx_01_01}; caseM = "swap";  end //  3 Swap Top 2 elements on stack
      urt4:   begin {ldV[3:0], selV[7:0]} = {4'b1111, 8'b01_11_10_11}; caseM = "urt4";  end //  4 Un-rotate 4
      nip :   begin {ldV[3:0], selV[7:0]} = {4'b0110, 8'bxx_11_10_xx}; caseM = "nip ";  end //  5 Remove second element from stack
      rot3:   begin {ldV[3:0], selV[7:0]} = {4'b0111, 8'bxx_10_01_10}; caseM = "rot3";  end //  6 Rotate top 3 elements on stack
      rot4:   begin {ldV[3:0], selV[7:0]} = {4'b1111, 8'b11_10_01_11}; caseM = "rot4";  end //  7 Rotate all 4 elements on stack
      exe1:   begin {ldV[3:0], selV[7:0]} = {4'b0001, 8'bxx_xx_xx_00}; caseM = "exe1";  end //  8 Replace V0
      exe2:   begin {ldV[3:0], selV[7:0]} = {4'b0111, 8'bxx_11_10_00}; caseM = "exe2";  end //  9 Pop 2 execute and Push 1
      exe3:   begin {ldV[3:0], selV[7:0]} = {4'b0111, 8'bxx_11_11_00}; caseM = "exe3";  end // 10 Pop 3 execute and Push 1
      exe4:   begin {ldV[3:0], selV[7:0]} = {4'b0011, 8'bxx_11_00_00}; caseM = "exe4";  end // 11 Pop 3 push 2 bit-serial add/sub
      exe5:   begin {ldV[3:0], selV[7:0]} = {4'b0111, 8'bxx_11_00_00}; caseM = "exe5";  end // 12 Pop 4 push 3 Bit-serial inc/dec
      exe6:   begin {ldV[3:0], selV[7:0]} = {4'b0011, 8'bxx_xx_00_00}; caseM = "exe6";  end // 13 Pop 2 push 2 Multiply
      exe7:   begin {ldV[3:0], selV[7:0]} = {4'b1111, 8'b11_10_00_00}; caseM = "exe7";  end // 14 Pop 1 push 2 Extend
      fill:   begin {ldV[3:0], selV[7:0]} = {4'b1000, 8'b00_xx_xx_xx}; caseM = "fill";  end // 15 Replace V3
      default begin {ldV[3:0], selV[7:0]} = {4'b0000, 8'bxx_xx_xx_xx}; caseM = "hold";  end //  0 'hold'
      endcase

wire  [  1:0]  sZ    = {ENwasm,ENsimd},
               sY    = {ENwasm & ~(Mctrl == rd),ENsimd | ENwasm & (Mctrl == rd)};
wire  [127:0]  Y,Z;
A2_mux #(2,128) iZ (.z(Z[127:0]), .s(sZ[1:0]), .d({ZW[127:0],  ZS[127:0]}));
A2_mux #(2,128) iY (.z(Y[127:0]), .s(sY[1:0]), .d({YW[127:0],  YS[127:0]}));

wire  [127:0]  nV0, nV1, nV2, nV3;
wire  [  3:0]  sV0   = 1 << selV[1:0];
wire  [  3:0]  sV1   = 1 << selV[3:2];
wire  [  3:0]  sV2   = 1 << selV[5:4];
wire  [  3:0]  sV3   = 1 << selV[7:6];

A2_mux #(4,128) iNV0 (.z(nV0[127:0]), .s(sV0[3:0]), .d({V3[127:0],  V2[127:0],  V1[127:0], Y[127:0]}));
A2_mux #(4,128) iNV1 (.z(nV1[127:0]), .s(sV1[3:0]), .d({V3[127:0],  V2[127:0],  V0[127:0], Z[127:0]}));
A2_mux #(4,128) iNV2 (.z(nV2[127:0]), .s(sV2[3:0]), .d({V3[127:0],  V1[127:0],  V0[127:0], Z[127:0]}));
A2_mux #(4,128) iNV3 (.z(nV3[127:0]), .s(sV3[3:0]), .d({V2[127:0],  V1[127:0],  V0[127:0], Y[127:0]}));

// BNN multiply is acheived with an XNOR gate.  So xnorscalar should enable MR ^~ iS[0]
// and conversely an inactive xnorscalar should allow simply MR
// Thus:
//     inc =   xnorscalar ? MR ~^ iS[0] : MR
//     inc =   MR ~^ (xnorscalar ? iS[0] : 1'b1)
//     inc =   MR ^  (xnorscalar & ~iS[0])
//

wire  [127:0]
   inc   =   MR[127:0] ^ {128{xnorscalar & ~iS_0}},
   cV0   =   inc,
   cV1   =   inc & V0[127:0],
   cV2   =   inc & V0[127:0] & V1[127:0],
   cV3   =   inc & V0[127:0] & V1[127:0] & V2[127:0],
   cEN   =   inc & V0[127:0] & V1[127:0] & V2[127:0] & V3[127:0];

// The Vector Stack -------------------------------------------------------------------------------------------------------------
always @(posedge clock) if (run && ldV[0] || mrak && !count) begin
                           for(i=0;i<16;i=i+1)
                              if (eLane[i] || ENsimd)
                                             V0[8*i+:8]  <= `tCQ  mrak ? MR[8*i+:8] : nV0[8*i+:8]; end
                   else if (count && mrak)   V0[127:0]   <= `tCQ  V0[127:0] ^ cV0[127:0];

always @(posedge clock) if (run && ldV[1])   V1[127:0]   <= `tCQ  nV1[127:0];
                   else if (count && mrak)   V1[127:0]   <= `tCQ  V1[127:0] ^ cV1[127:0];

always @(posedge clock) if (run && ldV[2])   V2[127:0]   <= `tCQ  nV2[127:0];
                   else if (count && mrak)   V2[127:0]   <= `tCQ  V2[127:0] ^ cV2[127:0];

always @(posedge clock) if (run && ldV[3])   V3[127:0]   <= `tCQ  nV3[127:0];
                   else if (count && mrak)   V3[127:0]   <= `tCQ  V3[127:0] ^ cV3[127:0];

// Memory Mask Enable -----------------------------------------------------------------------------------------------------------
always @(posedge clock) if (run &&   reset)  EN[127:0]   <= `tCQ   128'h0;
                   else if (run &&   clear)  EN[127:0]   <= `tCQ   128'h0;
                   else if (run &&   setEN)  EN[127:0]   <= `tCQ  {128{iS_0}};
                   else if (run &&  loadEN)  EN[127:0]   <= `tCQ   YS[127:0];
                   else if (run && writeEN)  EN[127:0]   <= `tCQ   V0[127:0];
                   else if (count &&  mrak)  EN[127:0]   <= `tCQ   cEN;
                   else if (run &&  flipEN)  EN[127:0]   <= `tCQ  ~EN[127:0];

endmodule //VectorRegs

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//                                ///////////////////////////////////////////////////////////////////////////////////////////////
// A2 CUSTOM SIMD                 ///////////////////////////////////////////////////////////////////////////////////////////////
//                                ///////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// NOTA BENE:
//    <<<<<<<<<<<<<<<<<<<<< NOT IMPLEMENTED >>>>>>>>>>>>>>>>>>>>>>>>
//    Repeat function loads scalar registers  'count' and 'length' and takes the next 'length' of instructions and repeats them
//    'count' times.  Writes to memory inside a loop are compressed into the previous instruction and delayed to the next
//    non-memory-read instruction (including another write).  For example a bit-serial add  (ld An) (ld Bn) (addc) (wr Cn) takes
//    the result of the addc and saves it together with the memory access parameters of the wr Cn.  As the addc is not changed
//    the result is still in V0.  So the (ld An) needs to be a (replace An). So an extra push is required in the start-up code
//    for the loop.
//    QED
//
//    Popcount turns V3..V0 into 128 4-bit counters so repeats are limited to 15.  So groups of 15 can be countes and followed by
//    adds of the group counts to complete the count when the length is greater than 15.
//
//    Spill and fill can be used to extend the effective stack depth.  Note that address pointer w has auto-increment and
//    auto-decrement; as opposed to pointers x,y & z that only have auto-increment.
//

module A2_custom_simd (
   output reg  [127:0]  YS, ZS,
   output reg  [ 31:0]  So,
   output reg  [  7:0]  Cctrl,
   output reg           setEN, loadEN, flipEN, writeEN, count, clear, xnorscalar, enEN,
   output reg           illegal_opcode,
   input  wire [127:0]  V0, V1, V2, V3, EN, MR,
   input  wire [ 31:0]  iS,
   input  wire [  7:0]  FN,
   input  wire          enable
   );

`ifdef   RELATIVE_FILENAMES                    // DEFINE IN SIMULATOR COMMAND LINE
   `include "A2Sv2r3_ISA.vh"
`else
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2Sv2r3_ISA.vh"           // ISA opcodes
`endif
// CONTROL_PARAMETERS -----------------------------------------------------------------------------------------------------------
localparam  [3:0]    hold = 4'd0,    push  = 4'd1,    pop  = 4'd2,    swap = 4'd3,
                     urt4 = 4'd4,    nip   = 4'd5,    rot3 = 4'd6,    rot4 = 4'd7,
                     exe1 = 4'd8,    exe2  = 4'd9,    exe3 = 4'd10,   exe4 = 4'd11,
                     exe5 = 4'd12,   exe6  = 4'd13,   exe7 = 4'd14,   fill = 4'd15;
localparam  [3:0]    lbV0 = 4'b1010, lbV1  = 4'b1100, lnot = 4'b0101,
                     land = 4'b1000, lxor  = 4'b0110, lior = 4'b1110, lann = 4'b0100;
localparam  [1:0]    hldS = 2'd0,    pushS = 2'd1,    popS = 2'd2;
localparam  [1:0]    no   = 2'd0,    rd    = 2'd1,    wr   = 2'd2;
localparam  [1:0]    Byte = 0,       Shrt  = 1,       Word = 2;
localparam  [1:0]    Right= 0,       Arith = 1,       Left = 2;
//-------------------------------------------------------------------------------------------------------------------------------

// These 2 functions cater to taking the result of upto 16 consecutive popcounts,  corner-turning the array and summing them
// into a 16 element vector of 8-bit unsigned values.  To get a single scalar result into A2S:
//    i16x8_extadd_pairwise_i8x16_u
//    i32x4_extadd_pairwise_i16x8_u
//    followed by 4 x  i32x4_extract_lane and 3 adds in A2S.   Maximum value is  128x16 => 2048,  Repeat this for larger sets!!
// We could sum the bytes in a vector to produce a single scalar output but not included in this version.

function [127:0] i8x16_orthogonal_sum;
input    [127:0] EN,V3,V2,V1,V0;
integer i;
for (i=0; i<16; i=i+1) begin
   i8x16_orthogonal_sum[8*i+:8] = orthogonal_sum( EN[8*i+:8],V3[8*i+:8],V2[8*i+:8],V1[8*i+:8],V0[8*i+:8] );
   end
endfunction

function [ 31:0] i8x16_vector_sum;
input    [127:0] V0;
integer i;
for (i=0; i<16; i=i+1) begin
   if (i==0) i8x16_vector_sum[31:0] = V0[8*i+:8];
   else      i8x16_vector_sum[31:0] = V0[8*i+:8] + i8x16_vector_sum[31:0];
   end
endfunction

function [7:0] orthogonal_sum;
input [7:0] en;
input [7:0] v3;
input [7:0] v2;
input [7:0] v1;
input [7:0] v0;
reg   [4:0] s0,s1,s2,s3,s4,s5,s6,s7;
   begin
   s0[4:0] = {en[0],v3[0],v2[0],v1[0],v0[0]};
   s1[4:0] = {en[1],v3[1],v2[1],v1[1],v0[1]};
   s2[4:0] = {en[2],v3[2],v2[2],v1[2],v0[2]};
   s3[4:0] = {en[3],v3[3],v2[3],v1[3],v0[3]};
   s4[4:0] = {en[4],v3[4],v2[4],v1[4],v0[4]};
   s5[4:0] = {en[5],v3[5],v2[5],v1[5],v0[5]};
   s6[4:0] = {en[6],v3[6],v2[6],v1[6],v0[6]};
   s7[4:0] = {en[7],v3[7],v2[7],v1[7],v0[7]};

   orthogonal_sum[7:0] = s0[4:0] + s1[4:0] + s2[4:0] + s3[4:0] + s4[4:0] + s5[4:0] + s6[4:0] + s7[4:0];
   end
endfunction


reg   [7:0] caseC;                  // debug only

always @* begin
   ZS             = 128'h0;
   YS             = 128'h0;
   So             =  32'h0;
   illegal_opcode =   1'b0;
   clear          =   1'b0;
   loadEN         =   1'b0;
   setEN          =   1'b0;
   writeEN        =   1'b0;
   flipEN         =   1'b0;
   enEN           =   1'b0;
   count          =   1'b0;
   xnorscalar     =   1'b0;
   if (enable)
      (* parallel_case *) casez (FN[7:0])
      // Load and Store ---------------------------------------------------------------------------------------------------------
      simd_clear           : begin                                clear  = 1'b1;          Cctrl = {hold,hldS,no}; caseC=100;  end
      simd_zero            : begin                                                        Cctrl = {push,hldS,no}; caseC=101;  end
      simd_ones            : begin  YS = {128{1'b1}};                                     Cctrl = {push,hldS,no}; caseC=102;  end
      simd_load_scalar     : begin  YS = {iS[31:0], V0[127:32]};                          Cctrl = {exe1,popS,no}; caseC=103;  end
      simd_store_scalar    : begin  YS = {V0[31:0], V0[127:32]};  So     = V0[31:0];      Cctrl = {exe1,pushS,no};caseC=104;  end
      // Memory Reads and Writes -----------------------------------------------------------------------------------------------
      simd_push            : begin                                                        Cctrl = {push,hldS,rd}; caseC=105;  end  // a b c d -- b c d e
      simd_replace         : begin                                                        Cctrl = {exe1,hldS,rd}; caseC=106;  end  // a b c d -- a b c e
      simd_popcount        : begin                                count  = 1'b1;          Cctrl = {hold,hldS,rd}; caseC=107;  end  // increment {V3:0} by WM[addr]
      simd_xnor_popcount   : begin             xnorscalar = 1'b1; count  = 1'b1;          Cctrl = {hold,hldS,rd}; caseC=108;  end  // increment {V3:0} by WM[addr] ^ {128{T0[0]}
      simd_load_EN         : begin  YS =  EN;                     loadEN = 1'b1;          Cctrl = {hold,hldS,rd}; caseC=109;  end  // EN = WM[addr]
      simd_store_EN        : begin  YS =  EN;                                             Cctrl = {hold,hldS,wr}; caseC=110;  end  // WM[addr] = EN
      simd_store_masked    : begin  YS =  V0;                     enEN   = 1'b1;          Cctrl = {pop, hldS,wr}; caseC=111;  end  // WM[addr] = EN ? v0 : WM[addr]
      simd_store           : begin  YS =  V0;                                             Cctrl = {pop, hldS,wr}; caseC=112;  end  // WM[addr] = v0
      // Bit-serial Add/Subtract/Increment/Decrement ---------------------------------------------------------------------------
      simd_bit_add         : begin  YS =  V0 ^ V1  ^  V2;
                                    ZS =  V0 & V1  |  V0  &  V2 |  V2 &  V1;              Cctrl = {exe4,hldS,no}; caseC=113;  end
      simd_bit_sub         : begin  YS = ~V0 ^ V1  ^  V2;
                                    ZS = ~V0 & V1  | ~V0  &  V2 |  V2 &  V1;              Cctrl = {exe4,hldS,no}; caseC=114;  end
      simd_bit_inc         : begin  YS =  V0 ^ V1  ^  V2;
                                    ZS =  V0 & V1  |  V0  &  V2 |  V2 &  V1;              Cctrl = {exe5,hldS,no}; caseC=115;  end
      simd_bit_dec         : begin  YS = ~V0 ^ V1  ^  V2;
                                    ZS = ~V0 & V1  | ~V0  &  V2 |  V2 &  V1;              Cctrl = {exe5,hldS,no}; caseC=116;  end
      simd_bit_add_sub     : begin  YS =  V0 ^ V1  ^  V2  ^  V3;
                                    ZS = (V0 ^ V3) &  V1  | (V0 ^ V3) &  V2 |  V2 & V3;   Cctrl = {exe4,hldS,no}; caseC=117;  end
      simd_bit_inc_dec     : begin  YS =  V0 ^ V1  ^  V2  ^  V3;
                                    ZS = (V0 ^ V3) &  V1  | (V0 ^ V3) &  V2 |  V2 & V3;   Cctrl = {exe5,hldS,no}; caseC=118;  end
      simd_merge           : begin  YS =  V0 & EN  | ~EN  &  V1;                          Cctrl = {exe2,hldS,rd}; caseC=119;  end
      simd_select          : begin  YS =  iS == 32'h0     ?  V0 : V1;                     Cctrl = {exe2,popS,no}; caseC=120;  end
      // Logical Operators -----------------------------------------------------------------------------------------------------
      simd_not             : begin  YS = ~V0;                                             Cctrl = {exe1,hldS,no}; caseC=121;  end
      simd_and             : begin  YS =  V0 & V1;                                        Cctrl = {exe2,hldS,no}; caseC=122;  end
      simd_ior             : begin  YS =  V0 | V1;                                        Cctrl = {exe2,hldS,no}; caseC=123;  end
      simd_xor             : begin  YS =  V0 ^ V1;                                        Cctrl = {exe2,hldS,no}; caseC=124;  end
      simd_bic             : begin  YS = ~V0 & V1;                                        Cctrl = {exe2,hldS,no}; caseC=125;  end
      // Stack Manipulation ----------------------------------------------------------------------------------------------------
      simd_swap            : begin                                                        Cctrl = {swap,hldS,no}; caseC=126;  end  // a b c d -- a b d c
      simd_drop            : begin                                                        Cctrl = { pop,hldS,no}; caseC=127;  end  // a b c d -- a a b c
      simd_nip             : begin                                                        Cctrl = { nip,hldS,no}; caseC=128;  end  // a b c d -- a a b d
      simd_dup             : begin  YS = V0;                                              Cctrl = {push,hldS,no}; caseC=129;  end  // a b c d -- b c d d
      simd_over            : begin  YS = V1;                                              Cctrl = {push,hldS,no}; caseC=130;  end  // a b c d -- b c d c
      simd_rotate3         : begin                                                        Cctrl = {rot3,hldS,no}; caseC=131;  end  // a b c d -- a c d b
      simd_rotate4         : begin                                                        Cctrl = {rot4,hldS,no}; caseC=132;  end  // a b c d -- b c d a
      simd_unrotate4       : begin                                                        Cctrl = {urt4,hldS,no}; caseC=133;  end  // a b c d -- d a b c
      // Memory Mask load and store --------------------------------------------------------------------------------------------
      simd_set_EN          : begin                                setEN  = 1'b1;          Cctrl = {hold,popS,no}; caseC=134;  end
      simd_read_EN         : begin  YS = EN;                                              Cctrl = {push,hldS,no}; caseC=135;  end
      simd_write_EN        : begin                                writeEN= 1'b1;          Cctrl = { pop,hldS,no}; caseC=136;  end
      simd_flip_EN         : begin                                flipEN = 1'b1;          Cctrl = {hold,hldS,no}; caseC=137;  end
      // Corner turn and sum popcounts
      simd_orthogonal_sum  : begin  YS = i8x16_orthogonal_sum(EN,V3,V2,V1,V0);            Cctrl = {exe1,hldS,no}; caseC=138;  end
      simd_vector_sum      : begin  So = i8x16_vector_sum(V0);                            Cctrl = {hold,hldS,no}; caseC=139;  end
      default begin illegal_opcode = 1'b1;                                                Cctrl = {hold,hldS,no}; caseC=254;  end
      //------------------------------------------------------------------------------------------------------------------------
      endcase
   else                      begin  YS = MR;                                              Cctrl = {hold,hldS,no}; caseC=255;  end
   end
endmodule //A2_custom_simd

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//                           ////////////////////////////////////////////////////////////////////////////////////////////////////
// WASM SIMD Decode          ////////////////////////////////////////////////////////////////////////////////////////////////////
//                           ////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

module   WASM_decode (
   output wire [102:0]  wasm_ctrl,
   output reg           illegal_opcode,
   input  wire [ 7:0]   FN,
   input  wire          ENwasm,
   input  wire          done
   );

`ifdef   RELATIVE_FILENAMES                    // DEFINE IN SIMULATOR COMMAND LINE
   `include "A2Sv2r3_ISA.vh"
`else
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2Sv2r3_ISA.vh"           // ISA opcodes
`endif

// CONTROL_PARAMETERS -----------------------------------------------------------------------------------------------------------
localparam  [3:0]    hold = 4'd0,    push  = 4'd1,    pop  = 4'd2,    swap = 4'd3,
                     urt4 = 4'd4,    nip   = 4'd5,    rot3 = 4'd6,    rot4 = 4'd7,
                     exe1 = 4'd8,    exe2  = 4'd9,    exe3 = 4'd10,   exe4 = 4'd11,
                     exe5 = 4'd12,   exe6  = 4'd13,   exe7 = 4'd14,   fill = 4'd15;
localparam  [3:0]    lbV0 = 4'b1010, lbV1  = 4'b1100, lnot = 4'b0101,
                     land = 4'b1000, lxor  = 4'b0110, lior = 4'b1110, lann = 4'b0100;
localparam  [1:0]    hldS = 2'd0,    pushS = 2'd1,    popS = 2'd2;
localparam  [1:0]    no   = 2'd0,    rd    = 2'd1,    wr   = 2'd2;
localparam  [1:0]    Byte = 0,       Shrt  = 1,       Word = 2;
localparam  [1:0]    Right= 0,       Arith = 1,       Left = 2;
//-------------------------------------------------------------------------------------------------------------------------------

wire [11:0] sSL;
wire  [7:0] sYA;
wire  [5:0] sXN;
wire  [5:0] sSI,  sSPL;
wire  [3:0] sEL,  sNR, truth;

reg   [7:0] Wctrl;
reg   [6:0] cc,   index;
reg   [1:0] tSH,  zSH;

wire        any_LBSS,   any_logic,  any_shift,  any_splat,  bitselect,  enableSM,   enableV1,
            abs8,       abs16,      abs32,      eq8,        eq16,       eq32,
            lm8,        lm16,       lm32,       ln8,        ln16,       ln32,
            lo8,        lo16,       lo32,       lt8,        lt16,       lt32,
            max8,       max16,      max32,      min8,       min16,      min32,
            neg8,       neg16,      neg32,      umn8,       umn16,      umn32,
            umx8,       umx16,      umx32,      ss8,        ss16,       us8,     us16,
            smax,       smin,       umax,       umin,       usad,
            output_scalar;

//wire [255:0] fd = {255'b0,ENwasm} << FN[7:0];
wire [255:0] fd = {255'b0,1'b1} << FN[7:0];

// Scalar mux selects
assign
   sSL[11]  =  fd[i8x16_all_true],
   sSL[10]  =  fd[i16x8_all_true],
   sSL[09]  =  fd[i32x4_all_true],
   sSL[08]  =  fd[v128_any_true],
   sSL[07]  =  fd[i32x4_bitmask],
   sSL[06]  =  fd[i16x8_bitmask],
   sSL[05]  =  fd[i8x16_bitmask],
   sSL[04]  =  fd[i8x16_extract_lane_u],
   sSL[03]  =  fd[i16x8_extract_lane_u],
   sSL[02]  =  fd[i8x16_extract_lane_s],
   sSL[01]  =  fd[i16x8_extract_lane_s],
   sSL[00]  =  fd[i32x4_extract_lane];

assign
   output_scalar  =  |sSL[11:0];

// Enable Lane selects
assign
   sEL[3]   =  fd[v128_load8_lane]  | fd[v128_store8_lane]  | fd[i8x16_replace_lane] | fd[i8x16_extract_lane_u] | fd[i8x16_extract_lane_s],
   sEL[2]   =  fd[v128_load16_lane] | fd[v128_store16_lane] | fd[i16x8_replace_lane] | fd[i16x8_extract_lane_u] | fd[i16x8_extract_lane_s],
   sEL[1]   =  fd[v128_load32_lane] | fd[v128_store32_lane] | fd[i32x4_replace_lane] | fd[i32x4_extract_lane],
   sEL[0]   = ~|sEL[3:1];

// SPLAT selects
assign
   sSPL[0]  =  fd[v128_load8_lane]  |  fd[i8x16_splat],
   sSPL[1]  =  fd[v128_load16_lane] |  fd[i16x8_splat],
   sSPL[2]  =  fd[v128_load32_lane] |  fd[i32x4_splat],
   sSPL[3]  =  fd[v128_load32_zero],
   sSPL[4]  =  fd[v128_const],
   sSPL[5]  = ~|sSPL[4:0];

// Arithmetic operation select
assign
   sYA[7]   =  fd[i8x16_eq]      |  fd[i16x8_eq]      |  fd[i32x4_eq]
            |  fd[i8x16_lt_s]    |  fd[i16x8_lt_s]    |  fd[i32x4_lt_s]
            |  fd[i8x16_lt_u]    |  fd[i16x8_lt_u]    |  fd[i32x4_lt_u],
   sYA[6]   =  fd[i8x16_max_s]   |  fd[i8x16_max_u]   |  fd[i8x16_min_s]   |  fd[i8x16_min_u]
            |  fd[i16x8_max_s]   |  fd[i16x8_max_u]   |  fd[i16x8_min_s]   |  fd[i16x8_min_u]
            |  fd[i32x4_max_s]   |  fd[i32x4_max_u]   |  fd[i32x4_min_s]   |  fd[i32x4_min_u],
   sYA[5]   =  fd[i8x16_avgr_u],
   sYA[4]   =  fd[i16x8_avgr_u],
   sYA[3]   =  fd[i32x4_extadd_pairwise_i16x8_s]      | fd[i32x4_extadd_pairwise_i16x8_u]
            |  fd[i16x8_extadd_pairwise_i8x16_u]      | fd[i16x8_extadd_pairwise_i8x16_s]
            |  fd[i8x16_add]     | fd[i16x8_add]      | fd[i32x4_add]      | fd[i8x16_sub]   | fd[i16x8_sub]   | fd[i32x4_sub],
   sYA[2]   =  fd[i8x16_popcnt],
   sYA[1]   =  fd[i32x4_extmul_dbl_i16x8_s]  | fd[i32x4_extmul_dbl_i16x8_u]
   `ifdef   ENABLE_DOT_PRODUCT
            |  fd[i32x4_dot_i16x8_s]
   `endif //ENABLE_DOT_PRODUCT
            |  fd[i16x8_extmul_dbl_i8x16_s]  |  fd[i16x8_extmul_dbl_i8x16_u]
            |  fd[i16x8_mul]                 |  fd[i32x4_mul]
            |  fd[i8x16_swizzle]             |  fd[i8x16_shuffle],
   sYA[0]   =  fd[i32x4_extend_dbl_i16x8_s]  |  fd[i32x4_extend_dbl_i16x8_u]
            |  fd[i16x8_extend_dbl_i8x16_s]  |  fd[i16x8_extend_dbl_i8x16_u]
            |  fd[i16x8_narrow_i32x4_u]      |  fd[i16x8_narrow_i32x4_s]
            |  fd[i8x16_narrow_i16x8_u]      |  fd[i8x16_narrow_i16x8_s];

`ifdef   ENABLE_NEGABSSAT
// Negate, Absolute & Saturation selects
wire [3:0]  sYB;
assign
   sYB[0]   =  fd[i8x16_neg]       |   fd[i16x8_neg]        |  fd[i32x4_neg],
   sYB[1]   =  fd[i8x16_abs]       |   fd[i16x8_abs]        |  fd[i32x4_abs],
   sYB[2]   =  fd[i16x8_add_sat_u] |   fd[i16x8_sub_sat_u]  |  fd[i8x16_add_sat_u]  |  fd[i8x16_sub_sat_u],
   sYB[3]   =  fd[i16x8_add_sat_s] |   fd[i16x8_sub_sat_s]  |  fd[i8x16_add_sat_s]  |  fd[i8x16_sub_sat_s];
`endif  //ENABLE_NEGABSSAT

// Select Small Immediates
assign
   sSI[0]   =  fd[i8x16_extract_lane_u],
   sSI[1]   =  fd[i8x16_extract_lane_s],
   sSI[2]   =  fd[i16x8_extract_lane_u],
   sSI[3]   =  fd[i16x8_extract_lane_s],
   sSI[4]   =  fd[i32x4_extract_lane  ];

// Final Selects

assign
   enableSM =  fd[i16x8_extmul_dbl_i8x16_s] | fd[i16x8_extmul_dbl_i8x16_u]
            |  fd[i32x4_extmul_dbl_i16x8_s] | fd[i32x4_extmul_dbl_i16x8_u]
            |  fd[i32x4_dot_i16x8_s];
assign
   enableV1 =  fd[i32x4_extend_dbl_i16x8_s] | fd[i16x8_extend_dbl_i8x16_s]
            |  fd[i32x4_extend_dbl_i16x8_u] | fd[i16x8_extend_dbl_i8x16_u];

assign
   sXN[5] =   fd[i16x8_narrow_i32x4_u]     | fd[i16x8_narrow_i32x4_s],
   sXN[4] =   fd[i8x16_narrow_i16x8_u]     | fd[i8x16_narrow_i16x8_s],
   sXN[3] =   fd[i32x4_extend_dbl_i16x8_s] | fd[i32x4_extend_dbl_i16x8_u]
          |   fd[i32x4_extmul_dbl_i16x8_s] | fd[i32x4_extmul_dbl_i16x8_u]
          |   fd[i32x4_extmul_dbl_i16x8_s] | fd[i32x4_extmul_dbl_i16x8_u]
          |   fd[i32x4_extend_dbl_i16x8_s] | fd[i32x4_extend_dbl_i16x8_u],

   sXN[2] =   fd[i16x8_extend_dbl_i8x16_s] | fd[i16x8_extend_dbl_i8x16_u]
          |   fd[i16x8_extmul_dbl_i8x16_s] | fd[i16x8_extmul_dbl_i8x16_u]
          |   fd[i16x8_extmul_dbl_i8x16_s] | fd[i16x8_extmul_dbl_i8x16_u]
          |   fd[i16x8_extend_dbl_i8x16_s] | fd[i16x8_extend_dbl_i8x16_u],
   sXN[1] =  ~sXN[5] & ~sXN[4] & any_LBSS,
   sXN[0] = ~|sXN[5:1];

// Create truth table for Logical operations
assign
   truth[3:0]  =  fd[v128_or]                ? lior
               :  fd[v128_xor]               ? lxor
               :  fd[v128_and]               ? land
               :  fd[v128_bitselect]
               || fd[v128_andnot]            ? lann
               :  fd[i16x8_narrow_i32x4_u]
               || fd[i16x8_narrow_i32x4_s]
               || fd[i8x16_narrow_i16x8_u]
               || fd[i8x16_narrow_i16x8_s]   ? lbV1
               :  fd[v128_not]               ? lnot : lbV0;
assign
   bitselect   =  fd[v128_bitselect];

assign
   any_logic   =  fd[v128_not]
               |  fd[v128_and]
               |  fd[v128_andnot]
               |  fd[v128_or]
               |  fd[v128_xor]
               |  fd[v128_bitselect]
               |  fd[i16x8_narrow_i32x4_u]
               |  fd[i16x8_narrow_i32x4_s]
               |  fd[i8x16_narrow_i16x8_u]
               |  fd[i8x16_narrow_i16x8_s];

// Create Shift control functions
always @* (* parallel_case *) case(1'b1)
   fd[i8x16_shl  ] : {zSH, tSH} = {Byte, Left };
   fd[i8x16_shr_s] : {zSH, tSH} = {Byte, Arith};
   fd[i8x16_shr_u] : {zSH, tSH} = {Byte, Right};
   fd[i16x8_shl  ] : {zSH, tSH} = {Shrt, Left };
   fd[i16x8_shr_s] : {zSH, tSH} = {Shrt, Arith};
   fd[i16x8_shr_u] : {zSH, tSH} = {Shrt, Right};
   fd[i32x4_shl  ] : {zSH, tSH} = {Word, Left };
   fd[i32x4_shr_s] : {zSH, tSH} = {Word, Arith};
   fd[i32x4_shr_u] : {zSH, tSH} = {Word, Right};
   default           {zSH, tSH} = {2'b11,2'b11};
   endcase

assign
   any_shift   = ~(zSH[1] & zSH[0] & tSH[1] & tSH[0]),
   any_splat   = ~sSPL[5],
   any_LBSS    =  any_logic | bitselect | any_shift | any_splat;

// Recode to control Adder\Subtractors -----------------------------------------------------------------------------------------
// Subtraction is done by inversion and carry in. From the ISA spec, 'a' is pushed first and 'b' is pushed second and 'b' is
// subtracted from 'a' so we have 'a' in V1 and 'b' on V0.  To subtract V1 - V0 we invert V0 and set carry in to a 1.
//
// Subtraction is also used for relational operators.  The less than relational is for b < a so we subtract b from a (as above)
// and see if the result goes negative.  For unsigned less than this is simply carry-out from the above 'subtraction'  For signed
// we must deal with negative values.  To this we XOR the carry out with the 'overflow' of the result.
//
// From the WASM spec: "The lt instructions, short for less than, check if a number is less than another number. If the first
// number 'a' is less than the second number 'b' equal (-1) will be pushed on to the stack, otherwise 0 will be pushed on to the
// stack." The first number 'a' is pushed to V0 and the second number 'b' pushes to V0 while V0 pushes to V1.
// So we check if V1 < V0 ('a' < 'b').
//
//  ___Code_______Meaning________________________________________________Flags Tested___________Boolean_______________________
//     eq         Equal.                                                   Z==1                  Z    Difference is zero
//     ne         Not equal.                                               Z==0                 ~Z
//     mi         Negative. The mnemonic stands for "minus".               N==1                  N    Sign of Difference
//     pl         Positive or zero. The mnemonic stands for "plus".        N==0                 ~N
//     vs         Signed overflow. The mnemonic stands for "V set".        V==1                  V    Overflow of Difference
//     vc         No signed overflow. The mnemonic stands for "V clear".   V==0                 ~V
//     cs or hs   Unsigned higher or same (or carry set).                  C==1                  C    Carry of Difference
//     cc or lo   Unsigned lower (or carry clear).                         C==0                 ~C
//     overflow   (~a[msb] & ~b[msb] & s[msb]) | ( a[msb] & b[msb] & ~s[msb] )

// Overflow if the sign of the result is different from the signs of the inputs.  If the input signs are different no overflow
// can occur:
// Signs of   a b sum  V N LT = V^N = sign_b & sign_sum
//            0 0 0    0 0 0
//            0 0 1    1 1 0
//            0 1 0    0 0 0
//            0 1 1    0 1 1
//            1 0 0    0 0 0
//            1 0 1    0 0 0
//            1 1 0    1 1 0
//            1 1 1    0 1 1
// Relational Operators:
// ___Code_____________________________________________________Combinational relations______________________________
//     ge         Signed greater than or equal.                  N==V                    N ^~ V
//     lt         Signed less than.                              N!=V                    N ^  V
//     gt         Signed greater than.                          (Z==0) && (N==V)        ~Z & (N ^~ V)
//     le         Signed less than or equal.                    (Z==1) || (N!=V)         Z | (N ^  V)
//     hi         Unsigned higher.                              (C==1) && (Z==0)         C & ~Z
//     ls         Unsigned lower or same.                       (C==0) || (Z==1)        ~C |  Z
// Overflow
// Signs    A B N    V  LT = ~A.B.N + A.~B.N + A.B.~N
//          0 0 0    0  0
//          0 0 1    1  0
//          0 1 0    0  0
//          0 1 1    0  1
//          1 0 0    0  0
//          1 0 1    0  1
//          1 1 0    1  1
//          1 1 1    0  0
//
// Create a value for each byte for each relational operator to use as the select of TRUE or FALSE
// decode relational operators
//
// The adders are partionable and are composed of 36-bit adders.  An additional bit
// is inserted on every byte boundary. These additional bits are used to kill the carry
// into the next more significant byte and/or to insert a carry into that byte.
// AB C KS
// 00 0 00 \__  S = carry out                    AB
// 00 1 01 /    K = no carry in                  0 0  BYTES DISCONNECTED
// 01 0 01 \__  K = carry through               01/10 BYTES CONNECTED
// 01 1 10 /    S = Not carry out - don't care   1 1  BYTES DISCONNECTED FORCE CARRY IN
// 10 0 01 \__  S = Not carry out - don't care
// 10 1 10 /    K = carry through
// 11 0 10 \__  K = force carry in                                               //    Byte 0 control -- carry in___________
// 11 1 11 /    S = carry out                                                    //    Byte 1 control____________________   |
//                                                                               //    Byte 2 control_________________   |  |
// Recode for add, subtract and carry in                                         //    Byte 3 control______________   |  |  |
                                                                                 //                                |  |  |  |
always @* (* parallel_case *) case(1'b1)                                         //                                |  |  |  |
   // Arithmetic  Add\Sub codes                                                  //                                AB AB AB A
`ifdef   ENABLE_NEGABSSAT
   fd[i8x16_add_sat_s],  fd[i8x16_add_sat_u]                                                       :  cc[6:0] = 7'b00_00_00_0;
   fd[i8x16_sub_sat_s],  fd[i8x16_sub_sat_u]                                                       :  cc[6:0] = 7'b11_11_11_1;
   fd[i16x8_add_sat_s],  fd[i16x8_add_sat_u]                                                       :  cc[6:0] = 7'b01_00_01_0;
   fd[i16x8_sub_sat_s],  fd[i16x8_sub_sat_u]                                                       :  cc[6:0] = 7'b01_11_01_1;
   fd[i8x16_abs],        fd[i8x16_neg]                                                             :  cc[6:0] = 7'b11_11_11_1;
   fd[i16x8_abs],        fd[i16x8_neg]                                                             :  cc[6:0] = 7'b01_11_01_1;
   fd[i32x4_abs],        fd[i32x4_neg]                                                             :  cc[6:0] = 7'b01_01_01_1;
`endif //ENABLE_NEGABSSAT
`ifdef   ENABLE_DOT_PRODUCT
   fd[i32x4_dot_i16x8_s]                                                                           :  cc[6:0] = 7'b01_11_01_1;
`endif //ENABLE_DOT_PRODUCT
   fd[i16x8_extmul_dbl_i8x16_s]                                                                    :  cc[6:0] = 7'b11_11_11_1;
   fd[i16x8_extmul_dbl_i8x16_u],
   fd[i32x4_extmul_dbl_i16x8_u]                                                                    :  cc[6:0] = 7'b01_00_01_0;
   fd[i32x4_extmul_dbl_i16x8_s]                                                                    :  cc[6:0] = 7'b01_11_01_1;

   fd[i8x16_add]                                                                                   :  cc[6:0] = 7'b00_00_00_0;
   fd[i16x8_add],    fd[i16x8_extadd_pairwise_i8x16_s],      fd[i16x8_extadd_pairwise_i8x16_u]     :  cc[6:0] = 7'b01_00_01_0;
   fd[i32x4_add],    fd[i32x4_extadd_pairwise_i16x8_s],      fd[i32x4_extadd_pairwise_i16x8_u]     :  cc[6:0] = 7'b01_01_01_0;

   fd[i8x16_sub],    fd[i8x16_avgr_u],
   fd[i8x16_min_s],  fd[i8x16_min_u], fd[i8x16_max_s],       fd[i8x16_max_u],
   fd[i8x16_eq],     fd[i8x16_lt_s],  fd[i8x16_lt_u]                                               :  cc[6:0] = 7'b11_11_11_1;

   fd[i16x8_sub],    fd[i16x8_avgr_u],
   fd[i16x8_min_s],  fd[i16x8_min_u], fd[i16x8_max_s],       fd[i16x8_max_u],
   fd[i16x8_eq],     fd[i16x8_lt_s],  fd[i16x8_lt_u]                                               :  cc[6:0] = 7'b01_11_01_1;

   fd[i32x4_sub],
   fd[i32x4_min_s],  fd[i32x4_min_u], fd[i32x4_max_s],       fd[i32x4_max_u],
   fd[i32x4_eq],     fd[i32x4_lt_s],  fd[i32x4_lt_u]                                               :  cc[6:0] = 7'b01_01_01_1;

   default                                                                                            cc[6:0] = 7'b00_00_00_0;
   endcase

// Relational Operators ---------------------------------------------------------------------------------------------------------
// NOTA BENE:  We abondoned all other relational operators to save space.  They can all be derived from these three anyway!!
assign
   eq8      = fd[i8x16_eq],      eq16  = fd[i16x8_eq],       eq32 = fd[i32x4_eq],
   lt8      = fd[i8x16_lt_s],    lt16  = fd[i16x8_lt_s],     lt32 = fd[i32x4_lt_s],
   lo8      = fd[i8x16_lt_u],    lo16  = fd[i16x8_lt_u],     lo32 = fd[i32x4_lt_u],
   abs8     = fd[i8x16_abs],     abs16 = fd[i16x8_abs],     abs32 = fd[i32x4_abs],
   neg8     = fd[i8x16_neg],     neg16 = fd[i16x8_neg],     neg32 = fd[i32x4_neg];

// Arithmetic Decode ------------------------------------------------------------------------------------------------------------
// decode saturation operators
assign
   ss8      =  fd[i8x16_add_sat_s] | fd[i8x16_sub_sat_s],
   ss16     =  fd[i16x8_add_sat_s] | fd[i16x8_sub_sat_s],
   us8      =  fd[i8x16_add_sat_u] | fd[i8x16_sub_sat_u],
   us16     =  fd[i16x8_add_sat_u] | fd[i16x8_sub_sat_u],
   usad     =  fd[i8x16_add_sat_u] | fd[i16x8_add_sat_u];

assign
   max8     =  fd[i8x16_max_s],
   max16    =  fd[i16x8_max_s],
   max32    =  fd[i32x4_max_s],
   min8     =  fd[i8x16_min_s],
   min16    =  fd[i16x8_min_s],
   min32    =  fd[i32x4_min_s],
   umx8     =  fd[i8x16_max_u],
   umx16    =  fd[i16x8_max_u],
   umx32    =  fd[i32x4_max_u],
   umn8     =  fd[i8x16_min_u],
   umn16    =  fd[i16x8_min_u],
   umn32    =  fd[i32x4_min_u];

assign
   smax     =  max8 | max16 | max32,
   smin     =  min8 | min16 | min32,
   umax     =  umx8 | umx16 | umx32,
   umin     =  umn8 | umn16 | umn32;

assign
   lm8   = lt8  |  max8   | min8,
   lm16  = lt16 |  max16  | min16,
   lm32  = lt32 |  max32  | min32,
   ln8   = lo8  |  umx8   | umn8,
   ln16  = lo16 |  umx16  | umn16,
   ln32  = lo32 |  umx32  | umn32;

// CONTROL DECODE
reg   [7:0] caseC;      // Debug only

assign
   sNR[0] = fd[i8x16_narrow_i16x8_u],
   sNR[1] = fd[i8x16_narrow_i16x8_s],
   sNR[2] = fd[i16x8_narrow_i32x4_u],
   sNR[3] = fd[i16x8_narrow_i32x4_s];

always @* begin
   illegal_opcode = 1'b0;
   (* parallel_case *) case (1'b1)
`ifdef   ENABLE_NARROW                                                              //            Vector Scalar Memory
      fd[i8x16_narrow_i16x8_u],  fd[i16x8_narrow_i32x4_u],
      fd[i8x16_narrow_i16x8_s],  fd[i16x8_narrow_i32x4_s]                           : begin  Wctrl = {exe2,hldS,no}; caseC=00; end
`else
      fd[i8x16_narrow_i16x8_u],  fd[i16x8_narrow_i32x4_u],
      fd[i8x16_narrow_i16x8_s],  fd[i16x8_narrow_i32x4_s]                           : begin  Wctrl = {exe6,hldS,no}; caseC=01; end
`endif //ENABLE_NARROW
`ifdef   ENABLE_DOT_PRODUCT
      fd[i32x4_dot_i16x8_s]                                                         : begin  Wctrl = {exe7,hldS,no}; caseC=02; end
`endif //ENABLE_DOT_PRODUCT
`ifdef   ENABLE_NEGABSSAT
      fd[i8x16_neg],             fd[i16x8_neg],             fd[i32x4_neg]           : begin  Wctrl = {exe1,hldS,no}; caseC=03; end
      fd[i8x16_abs],             fd[i16x8_abs],             fd[i32x4_abs]           : begin  Wctrl = {exe1,hldS,no}; caseC=04; end
      fd[i8x16_add_sat_s],       fd[i16x8_add_sat_s],
      fd[i8x16_sub_sat_s],       fd[i16x8_sub_sat_s],
      fd[i8x16_add_sat_u],       fd[i16x8_add_sat_u],
      fd[i8x16_sub_sat_u],       fd[i16x8_sub_sat_u]                                : begin  Wctrl = {exe2,hldS,no}; caseC=05; end
`endif //ENABLE_NEGABSSAT
      fd[i16x8_mul],                fd[i32x4_mul]                                   : begin  Wctrl = {exe2,hldS,no}; caseC=06; end
      fd[i16x8_extmul_dbl_i8x16_s], fd[i32x4_extmul_dbl_i16x8_s],
      fd[i16x8_extmul_dbl_i8x16_u], fd[i32x4_extmul_dbl_i16x8_u]                    : begin  Wctrl = {exe6,hldS,no}; caseC=07; end
      fd[v128_load]                                                                 : begin  Wctrl = {push,hldS,rd}; caseC=08; end
      fd[v128_const]                                                                : begin  Wctrl = {exe1,hldS,no}; caseC=09; end
      fd[v128_any_true]                                                             : begin  Wctrl = {pop,pushS,no}; caseC=10; end
      fd[v128_not]                                                                  : begin  Wctrl = {exe1,hldS,no}; caseC=11; end
      fd[v128_bitselect]                                                            : begin  Wctrl = {exe3,hldS,no}; caseC=12; end
      fd[v128_and],        fd[v128_xor],     fd[v128_or],   fd[v128_andnot]         : begin  Wctrl = {exe2,hldS,no}; caseC=13; end
      fd[v128_store]                                                                : begin  Wctrl = {pop, hldS,wr}; caseC=14; end
//    fd[v128_store8_lane],      fd[v128_store16_lane],     fd[v128_store32_lane]
//    fd[v128_load8_splat],      fd[v128_load16_splat],     fd[v128_load32_splat]   : begin  Wctrl = {push,hldS,no}; caseC=15; end
      fd[v128_load8_lane],       fd[v128_load16_lane],      fd[v128_load32_lane]    : begin  Wctrl = {exe1,popS,no}; caseC=16; end
      fd[v128_load32_zero]                                                          : begin  Wctrl = {push,popS,no}; caseC=16; end

      fd[i8x16_splat],           fd[i16x8_splat],           fd[i32x4_splat]         : begin  Wctrl = {push,popS,no}; caseC=17; end

      fd[i8x16_extract_lane_s],  fd[i16x8_extract_lane_s],  fd[i32x4_extract_lane],
      fd[i8x16_extract_lane_u],  fd[i16x8_extract_lane_u]                           : begin  Wctrl = {hold,pushS,no};caseC=18; end

      fd[i8x16_all_true],        fd[i16x8_all_true],        fd[i32x4_all_true],
      fd[i8x16_bitmask],         fd[i16x8_bitmask],         fd[i32x4_bitmask]       : begin  Wctrl = {pop,pushS,no}; caseC=19; end

      fd[i8x16_popcnt]                                                              : begin  Wctrl = {exe1,hldS,no}; caseC=20; end

      fd[i8x16_shl],             fd[i16x8_shl],             fd[i32x4_shl],
      fd[i8x16_shr_s],           fd[i16x8_shr_s],           fd[i32x4_shr_s],
      fd[i8x16_shr_u],           fd[i16x8_shr_u],           fd[i32x4_shr_u]         : begin  Wctrl = {exe1,popS,no}; caseC=21; end

      fd[i8x16_add],             fd[i16x8_add],             fd[i32x4_add],
      fd[i8x16_sub],             fd[i16x8_sub],             fd[i32x4_sub],
      fd[i8x16_avgr_u],          fd[i16x8_avgr_u],
      fd[i8x16_max_s],           fd[i16x8_max_s],           fd[i32x4_max_s],
      fd[i8x16_max_u],           fd[i16x8_max_u],           fd[i32x4_max_u],
      fd[i8x16_min_s],           fd[i16x8_min_s],           fd[i32x4_min_s],
      fd[i8x16_min_u],           fd[i16x8_min_u],           fd[i32x4_min_u],
      fd[i8x16_eq],              fd[i16x8_eq],              fd[i32x4_eq],
      fd[i8x16_lt_s],            fd[i16x8_lt_s],            fd[i32x4_lt_s],
      fd[i8x16_lt_u],            fd[i16x8_lt_u],            fd[i32x4_lt_u],
                                 fd[i16x8_mul],             fd[i32x4_mul]           : begin  Wctrl = {exe2,hldS,no}; caseC=22; end

      fd[i8x16_swizzle],
      fd[i8x16_shuffle]                                                 :  if (done)  begin  Wctrl = {exe3,hldS,no}; caseC=23; end
                                                                           else       begin  Wctrl = {hold,hldS,no}; caseC=24; end

      fd[i16x8_extadd_pairwise_i8x16_s],  fd[i32x4_extadd_pairwise_i16x8_s],
      fd[i16x8_extadd_pairwise_i8x16_u],  fd[i32x4_extadd_pairwise_i16x8_u]         : begin  Wctrl = {exe2,hldS,no}; caseC=25; end

      fd[i16x8_extend_dbl_i8x16_s],       fd[i32x4_extend_dbl_i16x8_s],
      fd[i16x8_extend_dbl_i8x16_u],       fd[i32x4_extend_dbl_i16x8_u]              : begin  Wctrl = {exe7,hldS,no}; caseC=26; end

      default  begin illegal_opcode = 1'b1;                                                  Wctrl = {hold,hldS,no}; caseC=27; end
      endcase
   end

// Output all the decodes -------------------------------------------------------------------------------------------------------
assign
   wasm_ctrl[102:0]  =  {Wctrl[7:0],     cc[6:0],  tSH[1:0],  zSH[1:0],   sSL[11:0],   sYA[7:0],  sYB[3:0],
                           sXN[5:0],   sSPL[5:0],  sEL[3:0],  sSI[4:0],   sNR[3:0],  truth[3:0],
                           any_LBSS,   any_logic,  any_shift, any_splat, bitselect,  enableSM,  enableV1,
                           abs8,       abs16,      abs32,
                           eq8,        eq16,       eq32,
                           lm8,        lm16,       lm32,
                           ln8,        ln16,       ln32,
                           neg8,       neg16,      neg32,
                           smax,       smin,       umax,      umin,
                           ss8,        ss16,       us8,       us16,  output_scalar};

endmodule //WASM_decode



/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//                 //////////////////////////////////////////////////////////////////////////////////////////////////////////////
//  VERIFICATION   //////////////////////////////////////////////////////////////////////////////////////////////////////////////
//                 //////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

`ifdef   verify_A2S_SIMD

module t;

parameter   FAST_PERIOD = 80;
parameter   XTRA_PERIOD = 200;
parameter   SLOW_PERIOD = 400;
parameter   SIMD_BASE   = 13'h0000;

integer i, cn, test, count;

localparam  [2:0]
   index_SIMD_CT  = 3'd0,
   index_SIMD_CF  = 3'd1,
   index_SIMD_WP  = 3'd2,
   index_SIMD_XP  = 3'd3,
   index_SIMD_YP  = 3'd4,
   index_SIMD_ZP  = 3'd5;

`include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2Sv2r3_ISA.vh"           // localparams defining ISA opcodes

// Outputs
wire [`IMISO_WIDTH-1:0]   imiso;         // Slave data out        33 {IOk, IOo[31:0]}
wire [`PMISO_WIDTH-1:0]   pmiso;         // SIMD Port to A2S      33 { SIMDex,SIMDir,     SIMDo[31:0]}
wire [`WMOSI_WIDTH-1:0]   wmosi;         // SIMD Port to WRAM    190 {eWM, wWM, iWM[127:0], mWM[127:0], aWM[31:0]}
reg  [`IMOSI_WIDTH-1:0]   imosi;         // Slave data in         48 {cIO[2:0] aIO[12:0], iIO[31:0]}
reg  [`PMOSI_WIDTH-1:0]   pmosi;         // SIMD Port to A2S      43 {eSIMD, cSIMD[9:0], iSIMD[31:0] }
wire [`WMISO_WIDTH-1:0]   wmiso;         // SIMD port to WRAM    130 {WMg, WMk, WMo[127:0]}
reg                       reset;
reg                       fast_clock;
reg                       slow_clock;

reg   [31:0] check;

A2Sv2r3_SIMD #(
   .BASE       (SIMD_BASE),
   .NUM_REGS   (6)
   ) mut (
   // imiso/imosi interface
   .imiso      (imiso),         // Slave data out        33 {IOk, IOo[31:0]}
   .imosi      (imosi),         // Slave data in         48 {cIO[2:0] aIO[12:0], iIO[31:0]}
   .pmiso      (pmiso),         // SIMD Port to A2S      33 { SIMDex,SIMDir,     SIMDo[31:0]}
   .pmosi      (pmosi),         // SIMD Port to A2S      43 {eSIMD, cSIMD[9:0], iSIMD[31:0] }
   .wmosi      (wmosi),         // SIMD Port to WRAM    190 {eWM, wWM, iWM[127:0], mWM[127:0], aWM[31:0]}
   .wmiso      (wmiso),         // SIMD port to WRAM    130 {WMg, WMk, WMo[127:0]}
   .reset      (reset),
   .fast_clock (fast_clock),
   .clock      (slow_clock)
   );

// Decollate buses --------------------------------------------

wire  [31:0]   SIMDo;
wire           SIMDex, SIMDir;
assign {SIMDex,SIMDir,SIMDo} = pmiso;

// Work Memory ------------------------------------------------

reg   [127:0]   WM [0:511];
wire  [127:0]  iWM, mWM;
reg   [127:0]   WMo;
wire  [ 31:0]  aWM;
wire           eWM, wWM, WMg;
reg             WMk;

assign {eWM, wWM, iWM[127:0], mWM[127:0], aWM[31:0]} = wmosi;

always @(posedge slow_clock)
   if (eWM)
      if (wWM) begin
         for (i=0; i<128; i=i+1)
            if (mWM[i])  WM[aWM[15:4]][i] <= `tCQ iWM[i];
         WMo <= `tCQ WM[aWM[15:4]];
         end
      else
         WMo <= `tCQ WM[aWM[15:4]];

always @(posedge slow_clock)
   WMk <= `tCQ eWM & ~wWM;

assign wmiso = {1'b1, WMk, WMo[127:0]};

// End Work Memory --------------------------------------------

//wire  [127:0]  shuffle_select = {mut.MPR[3].Mult.slice3.lower,
//                                 mut.MPR[3].Mult.slice2.lower,
//                                 mut.MPR[3].Mult.slice1.lower,
//                                 mut.MPR[3].Mult.slice0.lower,
//                                 mut.MPR[2].Mult.slice3.lower,
//                                 mut.MPR[2].Mult.slice2.lower,
//                                 mut.MPR[2].Mult.slice1.lower,
//                                 mut.MPR[2].Mult.slice0.lower,
//                                 mut.MPR[1].Mult.slice3.lower,
//                                 mut.MPR[1].Mult.slice2.lower,
//                                 mut.MPR[1].Mult.slice1.lower,
//                                 mut.MPR[1].Mult.slice0.lower,
//                                 mut.MPR[0].Mult.slice3.lower,
//                                 mut.MPR[0].Mult.slice2.lower,
//                                 mut.MPR[0].Mult.slice1.lower,
//                                 mut.MPR[0].Mult.slice0.lower};

initial begin
   fast_clock = 1'b0;
   forever fast_clock = #(FAST_PERIOD/2) ~fast_clock;
   end

initial begin
   slow_clock = 1'b0;
   forever slow_clock = #(SLOW_PERIOD/2) ~slow_clock;
   end

reg   xtra_clock;
initial begin
   xtra_clock = 1'b0;
   forever xtra_clock = #(XTRA_PERIOD/2) ~xtra_clock;
   end

always @(posedge slow_clock)
   if (reset) count <= 32'h0;
   else       count <= count + 1;

always @(posedge slow_clock)
   if (imiso[32]) check <= imiso[31:0];

initial begin
   reset =  1'b1;    // RTT 20240625
   imosi =  {`IMOSI_WIDTH{1'b0}};
   pmosi =  {`PMOSI_WIDTH{1'b0}};
// wmiso =  {`WMISO_WIDTH{1'b0}};
   repeat (10)  @(posedge slow_clock);
   reset =    1'b1;
   repeat (10)  @(posedge slow_clock);
   reset =    1'b0;
   repeat (100) @(posedge slow_clock);
   for (i=0; i<6; i=i+1) begin
      imosi =  {3'b110, 32'h7654_3210, 10'h0, i[2:0]};
      @(posedge slow_clock);
      imosi =   48'h0;
      @(posedge slow_clock); #1;
      imosi =  {3'b101, 32'h0000_0000, 10'h0, i[2:0]};
      @(posedge slow_clock); #1;
      if ( check !== 32'h7654_3210) $display("%d Error  expected 76543210 found %h",count,check);
      imosi =   48'h0;
      @(posedge slow_clock); #1;
      end
   for (i=0; i<6; i=i+1) begin
      imosi =  {3'b110, 32'h89ab_cdef, 10'h0, i[2:0]};
      @(posedge slow_clock);
      imosi =   48'h0;
      @(posedge slow_clock); #1;
      imosi =  {3'b101, 32'h0000_0000, 10'h0, i[2:0]};
      @(posedge slow_clock); #1;
      if ( check !== 32'h89ab_cdef) $display("%d Error  expected 89abcdef found %h",count,check);
      imosi =   48'h0;
      @(posedge slow_clock); #1;
      end

   imosi =  {3'b110, 32'h0043_3632, 10'h0, index_SIMD_CF};
   @(posedge slow_clock);
   imosi =   48'h0;
   @(posedge slow_clock); #1;
   imosi =  {3'b101, 32'h0000_0000, 10'h0, index_SIMD_CF};
   @(posedge slow_clock); #1;
   if ( check !== 32'h0043_3632) $display("%d Error  expected 00433632 found %h",count,check);
   imosi =   48'h0;
   @(posedge slow_clock); #1;

   @(posedge slow_clock);

   cn = -2;
   $display("%d Starting Vector Ops ",count);
   @(posedge slow_clock); #2;
   debugpush(127'h0f0e_0d0c_0b0a_0908_0706_0504_0302_0100);
   debugpush(127'h1f1e_1d1c_1b1a_1918_1716_1514_1312_1110);
   debugpush(127'h2f2e_2d2c_2b2a_2928_2726_2524_2322_2120);
   debugpush(127'h3f3e_3d3c_3b3a_3938_3736_3534_3332_3130);
   $display(" Vectors loaded \n V0 = %h \n V1 = %h \n V2 = %h \n V3 = %h \n", mut.iVR.V0,mut.V1,mut.V2,mut.V3);
   @(posedge slow_clock); #2;

   @(posedge slow_clock); #2;
   debugpush(128'h0000_0000_0000_0000_0000_0000_0000_0000);
   cn = -1;
   pmosi       = {3'b110,i8x16_all_true, 4'h0, 32'h0};
   @(posedge slow_clock); #2;
   pmosi       = 47'h0;
   if (SIMDo !== 32'h00000000) $display("%d Error  expected 00000000 %h",count,SIMDo);
   @(posedge slow_clock); #2;

   @(posedge slow_clock); #2;
   debugpush(128'h0101_0101_0101_0101_0101_0101_0101_0101);
   cn =  0;
   pmosi       = {3'b110,i8x16_all_true, 4'h0, 32'h0};
   @(posedge slow_clock); #2;
   pmosi       = 47'h0;
   if (SIMDo !== 32'hffffffff) $display("%d Error  expected ffffffff %h",count,SIMDo);
   @(posedge slow_clock); #2;

   $display(" Testing  v128_not");
   cn =  01; testcase_vv(v128_not,                 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn =  02; testcase_vv(v128_not,                 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn =  03; testcase_vv(v128_not,                 128'hffffffff00000000ffffffff00000000, 128'h00000000ffffffff00000000ffffffff);
   cn =  04; testcase_vv(v128_not,                 128'h00000000ffffffff00000000ffffffff, 128'hffffffff00000000ffffffff00000000);
   cn =  05; testcase_vv(v128_not,                 128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
   cn =  06; testcase_vv(v128_not,                 128'hcccccccccccccccccccccccccccccccc, 128'h33333333333333333333333333333333);
   cn =  07; testcase_vv(v128_not,                 128'h499602d2499602d2499602d2499602d2, 128'hb669fd2db669fd2db669fd2db669fd2d);
   cn =  08; testcase_vv(v128_not,                 128'h12345678123456781234567812345678, 128'hedcba987edcba987edcba987edcba987);

   $display(" Testing  all_true");
   cn =  09; testcase_vs(i8x16_all_true,           128'h00000000000000000000000000000000, 32'h00000000);
   cn =  10; testcase_vs(i8x16_all_true,           128'h00000000000000000000000000000100, 32'h00000000);
   cn =  11; testcase_vs(i8x16_all_true,           128'h01010101010101010101010101010001, 32'h00000000);
   cn =  12; testcase_vs(i8x16_all_true,           128'h01010101010101010101010101010101, 32'hffffffff); // Converted.  Was 00000001
   cn =  13; testcase_vs(i8x16_all_true,           128'hff000102030405060708090a0b0c0d0f, 32'h00000000);
   cn =  14; testcase_vs(i8x16_all_true,           128'h00000000000000000000000000000000, 32'h00000000);
   cn =  15; testcase_vs(i8x16_all_true,           128'hffffffffffffffffffffffffffffffff, 32'hffffffff);
   cn =  16; testcase_vs(i8x16_all_true,           128'habababababababababababababababab, 32'hffffffff);
   cn =  17; testcase_vs(i8x16_all_true,           128'h55555555555555555555555555555555, 32'hffffffff);

   cn =  18; testcase_vs(i16x8_all_true,           128'h00000000000000000000000000000000, 32'h00000000);
   cn =  19; testcase_vs(i16x8_all_true,           128'h00000000000000000000000000010000, 32'h00000000);
   cn =  20; testcase_vs(i16x8_all_true,           128'h00010001000100010001000100000001, 32'h00000000);
   cn =  21; testcase_vs(i16x8_all_true,           128'h00010001000100010001000100010001, 32'hffffffff);
   cn =  22; testcase_vs(i16x8_all_true,           128'hffff000000010002000b000c000d000f, 32'h00000000);
   cn =  23; testcase_vs(i16x8_all_true,           128'h00000000000000000000000000000000, 32'h00000000);
   cn =  24; testcase_vs(i16x8_all_true,           128'h00ff00ff00ff00ff00ff00ff00ff00ff, 32'hffffffff);
   cn =  25; testcase_vs(i16x8_all_true,           128'h00ab00ab00ab00ab00ab00ab00ab00ab, 32'hffffffff);
   cn =  26; testcase_vs(i16x8_all_true,           128'h00550055005500550055005500550055, 32'hffffffff);
   cn =  27; testcase_vs(i16x8_all_true,           128'h30393039303930393039303930393039, 32'hffffffff);
   cn =  28; testcase_vs(i16x8_all_true,           128'h12341234123412341234123412341234, 32'hffffffff);

   cn =  29; testcase_vs(i32x4_all_true,           128'h00000000000000000000000000000000, 32'h00000000);
   cn =  30; testcase_vs(i32x4_all_true,           128'h00000000000000000000000100000000, 32'h00000000);
   cn =  31; testcase_vs(i32x4_all_true,           128'h00000001000000010000000000000001, 32'h00000000);
   cn =  32; testcase_vs(i32x4_all_true,           128'h00000001000000010000000100000001, 32'hffffffff);
   cn =  33; testcase_vs(i32x4_all_true,           128'hffffffff00000000000000010000000f, 32'h00000000);
   cn =  34; testcase_vs(i32x4_all_true,           128'h00000000000000000000000000000000, 32'h00000000);
   cn =  35; testcase_vs(i32x4_all_true,           128'h000000ff000000ff000000ff000000ff, 32'hffffffff);
   cn =  36; testcase_vs(i32x4_all_true,           128'h000000ab000000ab000000ab000000ab, 32'hffffffff);
   cn =  37; testcase_vs(i32x4_all_true,           128'h00000055000000550000005500000055, 32'hffffffff);
   cn =  38; testcase_vs(i32x4_all_true,           128'h499602d2499602d2499602d2499602d2, 32'hffffffff);
   cn =  39; testcase_vs(i32x4_all_true,           128'h12345678123456781234567812345678, 32'hffffffff);

   $display(" Testing  v128_any_true");
   cn =  40; testcase_vs(v128_any_true,            128'h00000000000000000000000000000000, 32'h00000000);
   cn =  41; testcase_vs(v128_any_true,            128'h00000000000000000000000000000100, 32'hffffffff);
   cn =  42; testcase_vs(v128_any_true,            128'h01010101010101010101010101010001, 32'hffffffff);
   cn =  43; testcase_vs(v128_any_true,            128'hff000102030405060708090a0b0c0d0f, 32'hffffffff);
   cn =  44; testcase_vs(v128_any_true,            128'hffffffffffffffffffffffffffffffff, 32'hffffffff);
   cn =  45; testcase_vs(v128_any_true,            128'habababababababababababababababab, 32'hffffffff);
   cn =  46; testcase_vs(v128_any_true,            128'h55555555555555555555555555555555, 32'hffffffff);
   cn =  47; testcase_vs(v128_any_true,            128'h00010001000100010001000100000001, 32'hffffffff);
   cn =  48; testcase_vs(v128_any_true,            128'hffff000000010002000b000c000d000f, 32'hffffffff);
   cn =  49; testcase_vs(v128_any_true,            128'h00ff00ff00ff00ff00ff00ff00ff00ff, 32'hffffffff);
   cn =  50; testcase_vs(v128_any_true,            128'h00ab00ab00ab00ab00ab00ab00ab00ab, 32'hffffffff);
   cn =  51; testcase_vs(v128_any_true,            128'h00550055005500550055005500550055, 32'hffffffff);
   cn =  52; testcase_vs(v128_any_true,            128'h30393039303930393039303930393039, 32'hffffffff);
   cn =  53; testcase_vs(v128_any_true,            128'h12341234123412341234123412341234, 32'hffffffff);
   cn =  54; testcase_vs(v128_any_true,            128'h00000000000000000000000100000000, 32'hffffffff);
   cn =  55; testcase_vs(v128_any_true,            128'h00000001000000010000000000000001, 32'hffffffff);
   cn =  56; testcase_vs(v128_any_true,            128'h00000001000000010000000100000001, 32'hffffffff);
   cn =  57; testcase_vs(v128_any_true,            128'hffffffff00000000000000010000000f, 32'hffffffff);
   cn =  58; testcase_vs(v128_any_true,            128'h000000ff000000ff000000ff000000ff, 32'hffffffff);
   cn =  59; testcase_vs(v128_any_true,            128'h000000ab000000ab000000ab000000ab, 32'hffffffff);
   cn =  60; testcase_vs(v128_any_true,            128'h00000055000000550000005500000055, 32'hffffffff);
   cn =  61; testcase_vs(v128_any_true,            128'h499602d2499602d2499602d2499602d2, 32'hffffffff);
   cn =  62; testcase_vs(v128_any_true,            128'h12345678123456781234567812345678, 32'hffffffff);

   $display(" Testing  bitmask");
   cn =  63; testcase_vs(i8x16_bitmask,            128'hffffffffffffffffffffffffffffffff, 32'h0000ffff);
   cn =  64; testcase_vs(i8x16_bitmask,            128'hff_00_01_02_03_04_05_06_07_08_09_0a_0b_0c_0d_0f, 32'h00008000);
   cn =  65; testcase_vs(i16x8_bitmask,            128'hffffffffffffffffffffffffffffffff, 32'h000000ff);
   cn =  66; testcase_vs(i16x8_bitmask,            128'hffff000000010002000b000c000d000f, 32'h00000080);
   cn =  67; testcase_vs(i32x4_bitmask,            128'hffffffffffffffffffffffffffffffff, 32'h0000000f);
   cn =  68; testcase_vs(i32x4_bitmask,            128'hffffffff00000000000000010000000f, 32'h00000008);

   $display(" Testing  v128_and");
   cn =  69; testcase_vvv(v128_and,                128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h000000000000000000000000ffffffff);
   cn =  70; testcase_vvv(v128_and,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn =  71; testcase_vvv(v128_and,                128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn =  72; testcase_vvv(v128_and,                128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn =  73; testcase_vvv(v128_and,                128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001);
   cn =  74; testcase_vvv(v128_and,                128'h000000ff000000ff000000ff000000ff, 128'h00000055000000550000005500000055, 128'h00000055000000550000005500000055);
   cn =  75; testcase_vvv(v128_and,                128'h000000ff000000ff000000ff000000ff, 128'h00000080000000800000008000000080, 128'h00000080000000800000008000000080);
   cn =  76; testcase_vvv(v128_and,                128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h0000000a0000008000000005000000a5, 128'h0000000a0000008000000000000000a0);
   cn =  77; testcase_vvv(v128_and,                128'hffffffffffffffffffffffffffffffff, 128'h55555555555555555555555555555555, 128'h55555555555555555555555555555555);
   cn =  78; testcase_vvv(v128_and,                128'hffffffffffffffffffffffffffffffff, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
   cn =  79; testcase_vvv(v128_and,                128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn =  80; testcase_vvv(v128_and,                128'h55555555555555555555555555555555, 128'h000055550000ffff000055ff00005fff, 128'h00005555000055550000555500005555);
   cn =  81; testcase_vvv(v128_and,                128'h499602d2499602d2499602d2499602d2, 128'h499602d2499602d2499602d2499602d2, 128'h499602d2499602d2499602d2499602d2);
   cn =  82; testcase_vvv(v128_and,                128'h12345678123456781234567812345678, 128'h90abcdef90abcdef90abcdef90abcdef, 128'h10204468102044681020446810204468);
   $display(" Testing  v128_or");
   cn =  83; testcase_vvv(v128_or,                 128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h00000000ffffffffffffffffffffffff);
   cn =  84; testcase_vvv(v128_or,                 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn =  85; testcase_vvv(v128_or,                 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn =  86; testcase_vvv(v128_or,                 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn =  87; testcase_vvv(v128_or,                 128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001);
   cn =  88; testcase_vvv(v128_or,                 128'h000000ff000000ff000000ff000000ff, 128'h00000055000000550000005500000055, 128'h000000ff000000ff000000ff000000ff);
   cn =  89; testcase_vvv(v128_or,                 128'h000000ff000000ff000000ff000000ff, 128'h00000080000000800000008000000080, 128'h000000ff000000ff000000ff000000ff);
   cn =  90; testcase_vvv(v128_or,                 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h0000000a0000008000000005000000a5, 128'haaaaaaaaaaaaaaaaaaaaaaafaaaaaaaf);
   cn =  91; testcase_vvv(v128_or,                 128'hffffffffffffffffffffffffffffffff, 128'h55555555555555555555555555555555, 128'hffffffffffffffffffffffffffffffff);
   cn =  92; testcase_vvv(v128_or,                 128'hffffffffffffffffffffffffffffffff, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn =  93; testcase_vvv(v128_or,                 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn =  94; testcase_vvv(v128_or,                 128'h55555555555555555555555555555555, 128'h000055550000ffff000055ff00005fff, 128'h555555555555ffff555555ff55555fff);
   cn =  95; testcase_vvv(v128_or,                 128'h499602d2499602d2499602d2499602d2, 128'h499602d2499602d2499602d2499602d2, 128'h499602d2499602d2499602d2499602d2);
   cn =  96; testcase_vvv(v128_or,                 128'h12345678123456781234567812345678, 128'h90abcdef90abcdef90abcdef90abcdef, 128'h92bfdfff92bfdfff92bfdfff92bfdfff);
   $display(" Testing  v128_xor");
   cn =  97; testcase_vvv(v128_xor,                128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h00000000ffffffffffffffff00000000);
   cn =  98; testcase_vvv(v128_xor,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn =  99; testcase_vvv(v128_xor,                128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 100; testcase_vvv(v128_xor,                128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 101; testcase_vvv(v128_xor,                128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001, 128'h00000000000000000000000000000000);
   cn = 102; testcase_vvv(v128_xor,                128'h000000ff000000ff000000ff000000ff, 128'h00000055000000550000005500000055, 128'h000000aa000000aa000000aa000000aa);
   cn = 103; testcase_vvv(v128_xor,                128'h000000ff000000ff000000ff000000ff, 128'h00000080000000800000008000000080, 128'h0000007f0000007f0000007f0000007f);
   cn = 104; testcase_vvv(v128_xor,                128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h0000000a0000008000000005000000a5, 128'haaaaaaa0aaaaaa2aaaaaaaafaaaaaa0f);
   cn = 105; testcase_vvv(v128_xor,                128'hffffffffffffffffffffffffffffffff, 128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
   cn = 106; testcase_vvv(v128_xor,                128'hffffffffffffffffffffffffffffffff, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h55555555555555555555555555555555);
   cn = 107; testcase_vvv(v128_xor,                128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 108; testcase_vvv(v128_xor,                128'h55555555555555555555555555555555, 128'h000055550000ffff000055ff00005fff, 128'h555500005555aaaa555500aa55550aaa);
   cn = 109; testcase_vvv(v128_xor,                128'h499602d2499602d2499602d2499602d2, 128'h499602d2499602d2499602d2499602d2, 128'h00000000000000000000000000000000);
   cn = 110; testcase_vvv(v128_xor,                128'h12345678123456781234567812345678, 128'h90abcdef90abcdef90abcdef90abcdef, 128'h829f9b97829f9b97829f9b97829f9b97);
   $display(" Testing  v128_andnot");
   cn = 111; testcase_vvv(v128_andnot,             128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h0000000000000000ffffffff00000000);
   cn = 112; testcase_vvv(v128_andnot,             128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 113; testcase_vvv(v128_andnot,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 114; testcase_vvv(v128_andnot,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 115; testcase_vvv(v128_andnot,             128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001, 128'h00000000000000000000000000000000);
   cn = 116; testcase_vvv(v128_andnot,             128'h000000ff000000ff000000ff000000ff, 128'h00000055000000550000005500000055, 128'h000000aa000000aa000000aa000000aa);
   cn = 117; testcase_vvv(v128_andnot,             128'h000000ff000000ff000000ff000000ff, 128'h00000080000000800000008000000080, 128'h0000007f0000007f0000007f0000007f);
   cn = 118; testcase_vvv(v128_andnot,             128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h0000000a0000008000000005000000a5, 128'haaaaaaa0aaaaaa2aaaaaaaaaaaaaaa0a);
   cn = 119; testcase_vvv(v128_andnot,             128'hffffffffffffffffffffffffffffffff, 128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
   cn = 120; testcase_vvv(v128_andnot,             128'hffffffffffffffffffffffffffffffff, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h55555555555555555555555555555555);
   cn = 121; testcase_vvv(v128_andnot,             128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 122; testcase_vvv(v128_andnot,             128'h55555555555555555555555555555555, 128'h000055550000ffff000055ff00005fff, 128'h55550000555500005555000055550000);
   cn = 123; testcase_vvv(v128_andnot,             128'h499602d2499602d2499602d2499602d2, 128'h499602d2499602d2499602d2499602d2, 128'h00000000000000000000000000000000);
   cn = 124; testcase_vvv(v128_andnot,             128'h12345678123456781234567812345678, 128'h90abcdef90abcdef90abcdef90abcdef, 128'h02141210021412100214121002141210);

   $display(" Testing  shl");
   cn = 125; testcase_vsv(i8x16_shl,               128'h80c0000102030405060708090a0b0c0d, 32'h00000001, 128'h008000020406080a0c0e10121416181a);
   cn = 126; testcase_vsv(i8x16_shl,               128'haabbccddeeffa0b0c0d0e0f00a0b0c0d, 32'h00000004, 128'ha0b0c0d0e0f0000000000000a0b0c0d0);
   cn = 127; testcase_vsv(i8x16_shl,               128'h000102030405060708090a0b0c0d0e0f, 32'h00000008, 128'h000102030405060708090a0b0c0d0e0f);
   cn = 128; testcase_vsv(i8x16_shl,               128'h000102030405060708090a0b0c0d0e0f, 32'h00000020, 128'h000102030405060708090a0b0c0d0e0f);
   cn = 129; testcase_vsv(i8x16_shl,               128'h000102030405060708090a0b0c0d0e0f, 32'h00000080, 128'h000102030405060708090a0b0c0d0e0f);
   cn = 130; testcase_vsv(i8x16_shl,               128'h000102030405060708090a0b0c0d0e0f, 32'h00000100, 128'h000102030405060708090a0b0c0d0e0f);
   cn = 131; testcase_vsv(i8x16_shl,               128'h80c0000102030405060708090a0b0c0d, 32'h00000009, 128'h008000020406080a0c0e10121416181a);
   cn = 132; testcase_vsv(i8x16_shl,               128'h000102030405060708090a0b0c0d0e0f, 32'h00000009, 128'h00020406080a0c0e10121416181a1c1e);
   cn = 133; testcase_vsv(i8x16_shl,               128'h000102030405060708090a0b0c0d0e0f, 32'h00000011, 128'h00020406080a0c0e10121416181a1c1e);
   cn = 134; testcase_vsv(i8x16_shl,               128'h000102030405060708090a0b0c0d0e0f, 32'h00000021, 128'h00020406080a0c0e10121416181a1c1e);
   cn = 135; testcase_vsv(i8x16_shl,               128'h000102030405060708090a0b0c0d0e0f, 32'h00000081, 128'h00020406080a0c0e10121416181a1c1e);
   cn = 136; testcase_vsv(i8x16_shl,               128'h000102030405060708090a0b0c0d0e0f, 32'h00000101, 128'h00020406080a0c0e10121416181a1c1e);
   cn = 137; testcase_vsv(i8x16_shl,               128'h000102030405060708090a0b0c0d0e0f, 32'h00000201, 128'h00020406080a0c0e10121416181a1c1e);
   cn = 138; testcase_vsv(i8x16_shl,               128'h000102030405060708090a0b0c0d0e0f, 32'h00000202, 128'h0004080c1014181c2024282c3034383c);
   cn = 167; testcase_vsv(i16x8_shl,               128'hff80ffc0000000010002000300040005, 32'h00000001, 128'hff00ff8000000002000400060008000a);
   cn = 168; testcase_vsv(i16x8_shl,               128'h30393039303930393039303930393039, 32'h00000002, 128'hc0e4c0e4c0e4c0e4c0e4c0e4c0e4c0e4);
   cn = 160; testcase_vsv(i16x8_shl,               128'h12341234123412341234123412341234, 32'h00000002, 128'h48d048d048d048d048d048d048d048d0);
   cn = 161; testcase_vsv(i16x8_shl,               128'haabbccddeeffa0b0c0d0e0f00a0b0c0d, 32'h00000004, 128'habb0cdd0eff00b000d000f00a0b0c0d0);
   cn = 162; testcase_vsv(i16x8_shl,               128'h00000001000200030004000500060007, 32'h00000008, 128'h00000100020003000400050006000700);
   cn = 163; testcase_vsv(i16x8_shl,               128'h00000001000200030004000500060007, 32'h00000020, 128'h00000001000200030004000500060007);
   cn = 164; testcase_vsv(i16x8_shl,               128'h00000001000200030004000500060007, 32'h00000080, 128'h00000001000200030004000500060007);
   cn = 165; testcase_vsv(i16x8_shl,               128'h00000001000200030004000500060007, 32'h00000100, 128'h00000001000200030004000500060007);
   cn = 166; testcase_vsv(i16x8_shl,               128'hff80ffc0000000010002000300040005, 32'h00000011, 128'hff00ff8000000002000400060008000a);
   cn = 167; testcase_vsv(i16x8_shl,               128'h00000001000200030004000500060007, 32'h00000011, 128'h00000002000400060008000a000c000e);
   cn = 168; testcase_vsv(i16x8_shl,               128'h00000001000200030004000500060007, 32'h00000021, 128'h00000002000400060008000a000c000e);
   cn = 169; testcase_vsv(i16x8_shl,               128'h00000001000200030004000500060007, 32'h00000081, 128'h00000002000400060008000a000c000e);
   cn = 170; testcase_vsv(i16x8_shl,               128'h00000001000200030004000500060007, 32'h00000101, 128'h00000002000400060008000a000c000e);
   cn = 171; testcase_vsv(i16x8_shl,               128'h00000001000200030004000500060007, 32'h00000201, 128'h00000002000400060008000a000c000e);
   cn = 172; testcase_vsv(i16x8_shl,               128'h00000001000200030004000500060007, 32'h00000202, 128'h000000040008000c001000140018001c);
   cn = 203; testcase_vsv(i32x4_shl,               128'h80000000ffff8000000000000a0b0c0d, 32'h00000001, 128'h00000000ffff0000000000001416181a);
   cn = 204; testcase_vsv(i32x4_shl,               128'h499602d2499602d2499602d2499602d2, 32'h00000002, 128'h26580b4826580b4826580b4826580b48);
   cn = 205; testcase_vsv(i32x4_shl,               128'h12345678123456781234567812345678, 32'h00000002, 128'h48d159e048d159e048d159e048d159e0);
   cn = 206; testcase_vsv(i32x4_shl,               128'haabbccddeeffa0b0c0d0e0f00a0b0c0d, 32'h00000004, 128'habbccdd0effa0b000d0e0f00a0b0c0d0);
   cn = 207; testcase_vsv(i32x4_shl,               128'h00000000000000010000000e0000000f, 32'h00000008, 128'h000000000000010000000e0000000f00);
   cn = 208; testcase_vsv(i32x4_shl,               128'h00000000000000010000000e0000000f, 32'h00000020, 128'h00000000000000010000000e0000000f);
   cn = 209; testcase_vsv(i32x4_shl,               128'h00000000000000010000000e0000000f, 32'h00000080, 128'h00000000000000010000000e0000000f);
   cn = 210; testcase_vsv(i32x4_shl,               128'h00000000000000010000000e0000000f, 32'h00000100, 128'h00000000000000010000000e0000000f);
   cn = 211; testcase_vsv(i32x4_shl,               128'h80000000ffff8000000000000a0b0c0d, 32'h00000021, 128'h00000000ffff0000000000001416181a);
   cn = 212; testcase_vsv(i32x4_shl,               128'h00000000000000010000000e0000000f, 32'h00000021, 128'h00000000000000020000001c0000001e);
   cn = 213; testcase_vsv(i32x4_shl,               128'h00000000000000010000000e0000000f, 32'h00000041, 128'h00000000000000020000001c0000001e);
   cn = 214; testcase_vsv(i32x4_shl,               128'h00000000000000010000000e0000000f, 32'h00000081, 128'h00000000000000020000001c0000001e);
   cn = 215; testcase_vsv(i32x4_shl,               128'h00000000000000010000000e0000000f, 32'h00000101, 128'h00000000000000020000001c0000001e);
   cn = 216; testcase_vsv(i32x4_shl,               128'h00000000000000010000000e0000000f, 32'h00000201, 128'h00000000000000020000001c0000001e);
   cn = 217; testcase_vsv(i32x4_shl,               128'h00000000000000010000000e0000000f, 32'h00000202, 128'h0000000000000004000000380000003c);
   $display(" Testing  shr_u");
   cn = 139; testcase_vsv(i8x16_shr_u,             128'h80c0000102030405060708090a0b0c0d, 32'h00000001, 128'h40600000010102020303040405050606);
   cn = 140; testcase_vsv(i8x16_shr_u,             128'haabbccddeeffa0b0c0d0e0f00a0b0c0d, 32'h00000004, 128'h0a0b0c0d0e0f0a0b0c0d0e0f00000000);
   cn = 141; testcase_vsv(i8x16_shr_u,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000008, 128'h000102030405060708090a0b0c0d0e0f);
   cn = 142; testcase_vsv(i8x16_shr_u,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000020, 128'h000102030405060708090a0b0c0d0e0f);
   cn = 143; testcase_vsv(i8x16_shr_u,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000080, 128'h000102030405060708090a0b0c0d0e0f);
   cn = 144; testcase_vsv(i8x16_shr_u,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000100, 128'h000102030405060708090a0b0c0d0e0f);
   cn = 145; testcase_vsv(i8x16_shr_u,             128'h80c0000102030405060708090a0b0c0d, 32'h00000009, 128'h40600000010102020303040405050606);
   cn = 146; testcase_vsv(i8x16_shr_u,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000009, 128'h00000101020203030404050506060707);
   cn = 147; testcase_vsv(i8x16_shr_u,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000011, 128'h00000101020203030404050506060707);
   cn = 148; testcase_vsv(i8x16_shr_u,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000021, 128'h00000101020203030404050506060707);
   cn = 149; testcase_vsv(i8x16_shr_u,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000081, 128'h00000101020203030404050506060707);
   cn = 150; testcase_vsv(i8x16_shr_u,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000101, 128'h00000101020203030404050506060707);
   cn = 151; testcase_vsv(i8x16_shr_u,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000201, 128'h00000101020203030404050506060707);
   cn = 152; testcase_vsv(i8x16_shr_u,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000202, 128'h00000000010101010202020203030303);
   cn = 173; testcase_vsv(i16x8_shr_u,             128'hff80ffc0000000010002000300040005, 32'h00000001, 128'h7fc07fe0000000000001000100020002);
   cn = 174; testcase_vsv(i16x8_shr_u,             128'h30393039303930393039303930393039, 32'h00000002, 128'h0c0e0c0e0c0e0c0e0c0e0c0e0c0e0c0e);
   cn = 175; testcase_vsv(i16x8_shr_u,             128'h90ab90ab90ab90ab90ab90ab90ab90ab, 32'h00000002, 128'h242a242a242a242a242a242a242a242a);
   cn = 176; testcase_vsv(i16x8_shr_u,             128'haabbccddeeffa0b0c0d0e0f00a0b0c0d, 32'h00000004, 128'h0aab0ccd0eef0a0b0c0d0e0f00a000c0);
   cn = 177; testcase_vsv(i16x8_shr_u,             128'h00000001000200030004000500060007, 32'h00000008, 128'h00000000000000000000000000000000);
   cn = 178; testcase_vsv(i16x8_shr_u,             128'h00000001000200030004000500060007, 32'h00000020, 128'h00000001000200030004000500060007);
   cn = 179; testcase_vsv(i16x8_shr_u,             128'h00000001000200030004000500060007, 32'h00000080, 128'h00000001000200030004000500060007);
   cn = 180; testcase_vsv(i16x8_shr_u,             128'h00000001000200030004000500060007, 32'h00000100, 128'h00000001000200030004000500060007);
   cn = 181; testcase_vsv(i16x8_shr_u,             128'hff80ffc0000000010002000300040005, 32'h00000011, 128'h7fc07fe0000000000001000100020002);
   cn = 182; testcase_vsv(i16x8_shr_u,             128'h00000001000200030004000500060007, 32'h00000011, 128'h00000000000100010002000200030003);
   cn = 183; testcase_vsv(i16x8_shr_u,             128'h00000001000200030004000500060007, 32'h00000021, 128'h00000000000100010002000200030003);
   cn = 184; testcase_vsv(i16x8_shr_u,             128'h00000001000200030004000500060007, 32'h00000081, 128'h00000000000100010002000200030003);
   cn = 185; testcase_vsv(i16x8_shr_u,             128'h00000001000200030004000500060007, 32'h00000101, 128'h00000000000100010002000200030003);
   cn = 186; testcase_vsv(i16x8_shr_u,             128'h00000001000200030004000500060007, 32'h00000201, 128'h00000000000100010002000200030003);
   cn = 187; testcase_vsv(i16x8_shr_u,             128'h00000001000200030004000500060007, 32'h00000202, 128'h00000000000000000001000100010001);
   cn = 218; testcase_vsv(i32x4_shr_u,             128'h80000000ffff80000000000c0000000d, 32'h00000001, 128'h400000007fffc0000000000600000006);
   cn = 219; testcase_vsv(i32x4_shr_u,             128'h499602d2499602d2499602d2499602d2, 32'h00000002, 128'h126580b4126580b4126580b4126580b4);
   cn = 220; testcase_vsv(i32x4_shr_u,             128'h90abcdef90abcdef90abcdef90abcdef, 32'h00000002, 128'h242af37b242af37b242af37b242af37b);
   cn = 221; testcase_vsv(i32x4_shr_u,             128'haabbccddeeffa0b0c0d0e0f00a0b0c0d, 32'h00000004, 128'h0aabbccd0eeffa0b0c0d0e0f00a0b0c0);
   cn = 222; testcase_vsv(i32x4_shr_u,             128'h00000000000000010000000e0000000f, 32'h00000008, 128'h00000000000000000000000000000000);
   cn = 223; testcase_vsv(i32x4_shr_u,             128'h00000000000000010000000e0000000f, 32'h00000020, 128'h00000000000000010000000e0000000f);
   cn = 224; testcase_vsv(i32x4_shr_u,             128'h00000000000000010000000e0000000f, 32'h00000080, 128'h00000000000000010000000e0000000f);
   cn = 225; testcase_vsv(i32x4_shr_u,             128'h00000000000000010000000e0000000f, 32'h00000100, 128'h00000000000000010000000e0000000f);
   cn = 226; testcase_vsv(i32x4_shr_u,             128'h80000000ffff80000000000c0000000d, 32'h00000021, 128'h400000007fffc0000000000600000006);
   cn = 227; testcase_vsv(i32x4_shr_u,             128'h00000000000000010000000e0000000f, 32'h00000021, 128'h00000000000000000000000700000007);
   cn = 228; testcase_vsv(i32x4_shr_u,             128'h00000000000000010000000e0000000f, 32'h00000041, 128'h00000000000000000000000700000007);
   cn = 229; testcase_vsv(i32x4_shr_u,             128'h00000000000000010000000e0000000f, 32'h00000081, 128'h00000000000000000000000700000007);
   cn = 230; testcase_vsv(i32x4_shr_u,             128'h00000000000000010000000e0000000f, 32'h00000101, 128'h00000000000000000000000700000007);
   cn = 231; testcase_vsv(i32x4_shr_u,             128'h00000000000000010000000e0000000f, 32'h00000201, 128'h00000000000000000000000700000007);
   cn = 232; testcase_vsv(i32x4_shr_u,             128'h00000000000000010000000e0000000f, 32'h00000202, 128'h00000000000000000000000300000003);
   $display(" Testing  shr_s");
   cn = 153; testcase_vsv(i8x16_shr_s,             128'h80c0000102030405060708090a0b0c0d, 32'h00000001, 128'hc0e00000010102020303040405050606);
   cn = 154; testcase_vsv(i8x16_shr_s,             128'haabbccddeeffa0b0c0d0e0f00a0b0c0d, 32'h00000004, 128'hfafbfcfdfefffafbfcfdfeff00000000);
   cn = 155; testcase_vsv(i8x16_shr_s,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000008, 128'h000102030405060708090a0b0c0d0e0f);
   cn = 156; testcase_vsv(i8x16_shr_s,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000020, 128'h000102030405060708090a0b0c0d0e0f);
   cn = 157; testcase_vsv(i8x16_shr_s,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000080, 128'h000102030405060708090a0b0c0d0e0f);
   cn = 158; testcase_vsv(i8x16_shr_s,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000100, 128'h000102030405060708090a0b0c0d0e0f);
   cn = 159; testcase_vsv(i8x16_shr_s,             128'h80c0000102030405060708090a0b0c0d, 32'h00000009, 128'hc0e00000010102020303040405050606);
   cn = 160; testcase_vsv(i8x16_shr_s,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000009, 128'h00000101020203030404050506060707);
   cn = 161; testcase_vsv(i8x16_shr_s,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000011, 128'h00000101020203030404050506060707);
   cn = 162; testcase_vsv(i8x16_shr_s,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000021, 128'h00000101020203030404050506060707);
   cn = 163; testcase_vsv(i8x16_shr_s,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000081, 128'h00000101020203030404050506060707);
   cn = 164; testcase_vsv(i8x16_shr_s,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000101, 128'h00000101020203030404050506060707);
   cn = 165; testcase_vsv(i8x16_shr_s,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000201, 128'h00000101020203030404050506060707);
   cn = 166; testcase_vsv(i8x16_shr_s,             128'h000102030405060708090a0b0c0d0e0f, 32'h00000202, 128'h00000000010101010202020203030303);
   cn = 188; testcase_vsv(i16x8_shr_s,             128'hff80ffc0000000010002000300040005, 32'h00000001, 128'hffc0ffe0000000000001000100020002);
   cn = 189; testcase_vsv(i16x8_shr_s,             128'h30393039303930393039303930393039, 32'h00000002, 128'h0c0e0c0e0c0e0c0e0c0e0c0e0c0e0c0e);
   cn = 190; testcase_vsv(i16x8_shr_s,             128'h90ab90ab90ab90ab90ab90ab90ab90ab, 32'h00000002, 128'he42ae42ae42ae42ae42ae42ae42ae42a);
   cn = 191; testcase_vsv(i16x8_shr_s,             128'haabbccddeeffa0b0c0d0e0f00a0b0c0d, 32'h00000004, 128'hfaabfccdfeeffa0bfc0dfe0f00a000c0);
   cn = 192; testcase_vsv(i16x8_shr_s,             128'h00000001000200030004000500060007, 32'h00000008, 128'h00000000000000000000000000000000);
   cn = 193; testcase_vsv(i16x8_shr_s,             128'h00000001000200030004000500060007, 32'h00000020, 128'h00000001000200030004000500060007);
   cn = 194; testcase_vsv(i16x8_shr_s,             128'h00000001000200030004000500060007, 32'h00000080, 128'h00000001000200030004000500060007);
   cn = 195; testcase_vsv(i16x8_shr_s,             128'h00000001000200030004000500060007, 32'h00000100, 128'h00000001000200030004000500060007);
   cn = 196; testcase_vsv(i16x8_shr_s,             128'hff80ffc0000000010002000300040005, 32'h00000011, 128'hffc0ffe0000000000001000100020002);
   cn = 197; testcase_vsv(i16x8_shr_s,             128'h00000001000200030004000500060007, 32'h00000011, 128'h00000000000100010002000200030003);
   cn = 198; testcase_vsv(i16x8_shr_s,             128'h00000001000200030004000500060007, 32'h00000021, 128'h00000000000100010002000200030003);
   cn = 199; testcase_vsv(i16x8_shr_s,             128'h00000001000200030004000500060007, 32'h00000081, 128'h00000000000100010002000200030003);
   cn = 200; testcase_vsv(i16x8_shr_s,             128'h00000001000200030004000500060007, 32'h00000101, 128'h00000000000100010002000200030003);
   cn = 201; testcase_vsv(i16x8_shr_s,             128'h00000001000200030004000500060007, 32'h00000201, 128'h00000000000100010002000200030003);
   cn = 202; testcase_vsv(i16x8_shr_s,             128'h00000001000200030004000500060007, 32'h00000202, 128'h00000000000000000001000100010001);
   cn = 233; testcase_vsv(i32x4_shr_s,             128'h80000000ffff80000000000c0000000d, 32'h00000001, 128'hc0000000ffffc0000000000600000006);
   cn = 234; testcase_vsv(i32x4_shr_s,             128'h499602d2499602d2499602d2499602d2, 32'h00000002, 128'h126580b4126580b4126580b4126580b4);
   cn = 235; testcase_vsv(i32x4_shr_s,             128'h90abcdef90abcdef90abcdef90abcdef, 32'h00000002, 128'he42af37be42af37be42af37be42af37b);
   cn = 236; testcase_vsv(i32x4_shr_s,             128'haabbccddeeffa0b0c0d0e0f00a0b0c0d, 32'h00000004, 128'hfaabbccdfeeffa0bfc0d0e0f00a0b0c0);
   cn = 237; testcase_vsv(i32x4_shr_s,             128'h00000000000000010000000e0000000f, 32'h00000008, 128'h00000000000000000000000000000000);
   cn = 238; testcase_vsv(i32x4_shr_s,             128'h00000000000000010000000e0000000f, 32'h00000020, 128'h00000000000000010000000e0000000f);
   cn = 239; testcase_vsv(i32x4_shr_s,             128'h00000000000000010000000e0000000f, 32'h00000080, 128'h00000000000000010000000e0000000f);
   cn = 240; testcase_vsv(i32x4_shr_s,             128'h00000000000000010000000e0000000f, 32'h00000100, 128'h00000000000000010000000e0000000f);
   cn = 241; testcase_vsv(i32x4_shr_s,             128'h80000000ffff80000000000c0000000d, 32'h00000021, 128'hc0000000ffffc0000000000600000006);
   cn = 242; testcase_vsv(i32x4_shr_s,             128'h00000000000000010000000e0000000f, 32'h00000021, 128'h00000000000000000000000700000007);
   cn = 243; testcase_vsv(i32x4_shr_s,             128'h00000000000000010000000e0000000f, 32'h00000041, 128'h00000000000000000000000700000007);
   cn = 244; testcase_vsv(i32x4_shr_s,             128'h00000000000000010000000e0000000f, 32'h00000081, 128'h00000000000000000000000700000007);
   cn = 245; testcase_vsv(i32x4_shr_s,             128'h00000000000000010000000e0000000f, 32'h00000101, 128'h00000000000000000000000700000007);
   cn = 246; testcase_vsv(i32x4_shr_s,             128'h00000000000000010000000e0000000f, 32'h00000201, 128'h00000000000000000000000700000007);
   cn = 247; testcase_vsv(i32x4_shr_s,             128'h00000000000000010000000e0000000f, 32'h00000202, 128'h00000000000000000000000300000003);

   $display(" Testing  v128_bitselect");
   cn = 248; testcase_vvvv(v128_bitselect,         128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, 128'h00112345f00fffff10112021bbaabbaa, 128'hbbaababaabbaaaaaabaabbbaaabbaabb);
   cn = 249; testcase_vvvv(v128_bitselect,         128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, 128'h00000000000000000000000000000000, 128'hbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
   cn = 250; testcase_vvvv(v128_bitselect,         128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, 128'h11111111111111111111111111111111, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
   cn = 251; testcase_vvvv(v128_bitselect,         128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, 128'h0123456789abcdeffedcba9876543210, 128'hbabababababababaabababababababab);
   cn = 252; testcase_vvvv(v128_bitselect,         128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h55555555555555555555555555555555, 128'h0123456789abcdeffedcba9876543210, 128'h54761032dcfe98baab89efcd23016745);
   cn = 253; testcase_vvvv(v128_bitselect,         128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h55555555555555555555555555555555, 128'h55555555aaaaaaaa00000000ffffffff, 128'h00000000ffffffff55555555aaaaaaaa);
   cn = 254; testcase_vvvv(v128_bitselect,         128'h499602d2499602d2499602d2499602d2, 128'hb669fd2eb669fd2eb669fd2eb669fd2e, 128'hcdefcdefcdefcdefcdefcdefcdefcdef, 128'h7b8630c27b8630c27b8630c27b8630c2);
   cn = 255; testcase_vvvv(v128_bitselect,         128'h12345678123456781234567812345678, 128'h90abcdef90abcdef90abcdef90abcdef, 128'hcdefcdefcdefcdefcdefcdefcdefcdef, 128'h10244468102444681024446810244468);
`ifdef   ENABLE_NARROW
   $display(" Testing  narrow");
   cn = 256; testcase_vvv(i8x16_narrow_i16x8_s,    128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 257; testcase_vvv(i8x16_narrow_i16x8_s,    128'h00000000000000000000000000000000, 128'h00010001000100010001000100010001, 128'h00000000000000000101010101010101);
   cn = 258; testcase_vvv(i8x16_narrow_i16x8_s,    128'h00010001000100010001000100010001, 128'h00000000000000000000000000000000, 128'h01010101010101010000000000000000);
   cn = 259; testcase_vvv(i8x16_narrow_i16x8_s,    128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h0000000000000000ffffffffffffffff);
   cn = 260; testcase_vvv(i8x16_narrow_i16x8_s,    128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffff0000000000000000);
   cn = 261; testcase_vvv(i8x16_narrow_i16x8_s,    128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff, 128'h0101010101010101ffffffffffffffff);
   cn = 262; testcase_vvv(i8x16_narrow_i16x8_s,    128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'hffffffffffffffff0101010101010101);
   cn = 263; testcase_vvv(i8x16_narrow_i16x8_s,    128'h007e007e007e007e007e007e007e007e, 128'h007f007f007f007f007f007f007f007f, 128'h7e7e7e7e7e7e7e7e7f7f7f7f7f7f7f7f);
   cn = 264; testcase_vvv(i8x16_narrow_i16x8_s,    128'h007f007f007f007f007f007f007f007f, 128'h007e007e007e007e007e007e007e007e, 128'h7f7f7f7f7f7f7f7f7e7e7e7e7e7e7e7e);
   cn = 265; testcase_vvv(i8x16_narrow_i16x8_s,    128'h007f007f007f007f007f007f007f007f, 128'h007f007f007f007f007f007f007f007f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 266; testcase_vvv(i8x16_narrow_i16x8_s,    128'h00800080008000800080008000800080, 128'h00800080008000800080008000800080, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 267; testcase_vvv(i8x16_narrow_i16x8_s,    128'h007f007f007f007f007f007f007f007f, 128'h00800080008000800080008000800080, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 268; testcase_vvv(i8x16_narrow_i16x8_s,    128'h00800080008000800080008000800080, 128'h007f007f007f007f007f007f007f007f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 269; testcase_vvv(i8x16_narrow_i16x8_s,    128'h007f007f007f007f007f007f007f007f, 128'h00ff00ff00ff00ff00ff00ff00ff00ff, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 270; testcase_vvv(i8x16_narrow_i16x8_s,    128'h00ff00ff00ff00ff00ff00ff00ff00ff, 128'h007f007f007f007f007f007f007f007f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 271; testcase_vvv(i8x16_narrow_i16x8_s,    128'hff81ff81ff81ff81ff81ff81ff81ff81, 128'hff80ff80ff80ff80ff80ff80ff80ff80, 128'h81818181818181818080808080808080);
   cn = 272; testcase_vvv(i8x16_narrow_i16x8_s,    128'hff80ff80ff80ff80ff80ff80ff80ff80, 128'hff81ff81ff81ff81ff81ff81ff81ff81, 128'h80808080808080808181818181818181);
   cn = 273; testcase_vvv(i8x16_narrow_i16x8_s,    128'hff80ff80ff80ff80ff80ff80ff80ff80, 128'hff80ff80ff80ff80ff80ff80ff80ff80, 128'h80808080808080808080808080808080);
   cn = 274; testcase_vvv(i8x16_narrow_i16x8_s,    128'hff7fff7fff7fff7fff7fff7fff7fff7f, 128'hff7fff7fff7fff7fff7fff7fff7fff7f, 128'h80808080808080808080808080808080);
   cn = 275; testcase_vvv(i8x16_narrow_i16x8_s,    128'hff7fff7fff7fff7fff7fff7fff7fff7f, 128'hff80ff80ff80ff80ff80ff80ff80ff80, 128'h80808080808080808080808080808080);
   cn = 276; testcase_vvv(i8x16_narrow_i16x8_s,    128'hff80ff80ff80ff80ff80ff80ff80ff80, 128'hff7fff7fff7fff7fff7fff7fff7fff7f, 128'h80808080808080808080808080808080);
   cn = 277; testcase_vvv(i8x16_narrow_i16x8_s,    128'hff80ff80ff80ff80ff80ff80ff80ff80, 128'h007f007f007f007f007f007f007f007f, 128'h80808080808080807f7f7f7f7f7f7f7f);
   cn = 278; testcase_vvv(i8x16_narrow_i16x8_s,    128'hff80ff80ff80ff80ff80ff80ff80ff80, 128'h00800080008000800080008000800080, 128'h80808080808080807f7f7f7f7f7f7f7f);
   cn = 279; testcase_vvv(i8x16_narrow_i16x8_s,    128'hff7fff7fff7fff7fff7fff7fff7fff7f, 128'h007f007f007f007f007f007f007f007f, 128'h80808080808080807f7f7f7f7f7f7f7f);
   cn = 280; testcase_vvv(i8x16_narrow_i16x8_s,    128'hff80ff80ff80ff80ff80ff80ff80ff80, 128'h00ff00ff00ff00ff00ff00ff00ff00ff, 128'h80808080808080807f7f7f7f7f7f7f7f);
   cn = 281; testcase_vvv(i8x16_narrow_i16x8_s,    128'hff7fff7fff7fff7fff7fff7fff7fff7f, 128'h01000100010001000100010001000100, 128'h80808080808080807f7f7f7f7f7f7f7f);
   cn = 282; testcase_vvv(i8x16_narrow_i16x8_s,    128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff, 128'h8080808080808080ffffffffffffffff);
   cn = 283; testcase_vvv(i8x16_narrow_i16x8_s,    128'h30393039303930393039303930393039, 128'hddd5ddd5ddd5ddd5ddd5ddd5ddd5ddd5, 128'h7f7f7f7f7f7f7f7f8080808080808080);
   cn = 284; testcase_vvv(i8x16_narrow_i16x8_s,    128'h12341234123412341234123412341234, 128'h56785678567856785678567856785678, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 285; testcase_vvv(i8x16_narrow_i16x8_u,    128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 286; testcase_vvv(i8x16_narrow_i16x8_u,    128'h00000000000000000000000000000000, 128'h00010001000100010001000100010001, 128'h00000000000000000101010101010101);
   cn = 287; testcase_vvv(i8x16_narrow_i16x8_u,    128'h00010001000100010001000100010001, 128'h00000000000000000000000000000000, 128'h01010101010101010000000000000000);
   cn = 288; testcase_vvv(i8x16_narrow_i16x8_u,    128'h0000_0000_0000_0000_0000_0000_0000_0000, 128'hffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff, 128'h0000_0000_0000_0000_0000_0000_0000_0000);
   cn = 289; testcase_vvv(i8x16_narrow_i16x8_u,    128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 290; testcase_vvv(i8x16_narrow_i16x8_u,    128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010000000000000000);
   cn = 291; testcase_vvv(i8x16_narrow_i16x8_u,    128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'h00000000000000000101010101010101);
   cn = 292; testcase_vvv(i8x16_narrow_i16x8_u,    128'h007e007e007e007e007e007e007e007e, 128'h007f007f007f007f007f007f007f007f, 128'h7e7e7e7e7e7e7e7e7f7f7f7f7f7f7f7f);
   cn = 293; testcase_vvv(i8x16_narrow_i16x8_u,    128'h007f007f007f007f007f007f007f007f, 128'h007e007e007e007e007e007e007e007e, 128'h7f7f7f7f7f7f7f7f7e7e7e7e7e7e7e7e);
   cn = 294; testcase_vvv(i8x16_narrow_i16x8_u,    128'h007f007f007f007f007f007f007f007f, 128'h007f007f007f007f007f007f007f007f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 295; testcase_vvv(i8x16_narrow_i16x8_u,    128'h00800080008000800080008000800080, 128'h00800080008000800080008000800080, 128'h80808080808080808080808080808080);
   cn = 296; testcase_vvv(i8x16_narrow_i16x8_u,    128'h007f007f007f007f007f007f007f007f, 128'h00800080008000800080008000800080, 128'h7f7f7f7f7f7f7f7f8080808080808080);
   cn = 297; testcase_vvv(i8x16_narrow_i16x8_u,    128'h00800080008000800080008000800080, 128'h007f007f007f007f007f007f007f007f, 128'h80808080808080807f7f7f7f7f7f7f7f);
   cn = 298; testcase_vvv(i8x16_narrow_i16x8_u,    128'h00fe00fe00fe00fe00fe00fe00fe00fe, 128'h00ff00ff00ff00ff00ff00ff00ff00ff, 128'hfefefefefefefefeffffffffffffffff);
   cn = 299; testcase_vvv(i8x16_narrow_i16x8_u,    128'h00ff00ff00ff00ff00ff00ff00ff00ff, 128'h00fe00fe00fe00fe00fe00fe00fe00fe, 128'hfffffffffffffffffefefefefefefefe);
   cn = 300; testcase_vvv(i8x16_narrow_i16x8_u,    128'h00ff00ff00ff00ff00ff00ff00ff00ff, 128'h00ff00ff00ff00ff00ff00ff00ff00ff, 128'hffffffffffffffffffffffffffffffff);
   cn = 301; testcase_vvv(i8x16_narrow_i16x8_u,    128'h01000100010001000100010001000100, 128'h01000100010001000100010001000100, 128'hffffffffffffffffffffffffffffffff);
   cn = 302; testcase_vvv(i8x16_narrow_i16x8_u,    128'h00ff00ff00ff00ff00ff00ff00ff00ff, 128'h01000100010001000100010001000100, 128'hffffffffffffffffffffffffffffffff);
   cn = 303; testcase_vvv(i8x16_narrow_i16x8_u,    128'h01000100010001000100010001000100, 128'h00ff00ff00ff00ff00ff00ff00ff00ff, 128'hffffffffffffffffffffffffffffffff);
   cn = 304; testcase_vvv(i8x16_narrow_i16x8_u,    128'h00000000000000000000000000000000, 128'h00ff00ff00ff00ff00ff00ff00ff00ff, 128'h0000000000000000ffffffffffffffff);
   cn = 305; testcase_vvv(i8x16_narrow_i16x8_u,    128'h00000000000000000000000000000000, 128'h01000100010001000100010001000100, 128'h0000000000000000ffffffffffffffff);
   cn = 306; testcase_vvv(i8x16_narrow_i16x8_u,    128'hffffffffffffffffffffffffffffffff, 128'h00ff00ff00ff00ff00ff00ff00ff00ff, 128'h0000000000000000ffffffffffffffff);
   cn = 307; testcase_vvv(i8x16_narrow_i16x8_u,    128'hffffffffffffffffffffffffffffffff, 128'h01000100010001000100010001000100, 128'h0000000000000000ffffffffffffffff);
   cn = 308; testcase_vvv(i8x16_narrow_i16x8_u,    128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 309; testcase_vvv(i8x16_narrow_i16x8_u,    128'hddd5ddd5ddd5ddd5ddd5ddd5ddd5ddd5, 128'h30393039303930393039303930393039, 128'h0000000000000000ffffffffffffffff);
   cn = 310; testcase_vvv(i8x16_narrow_i16x8_u,    128'h90ab90ab90ab90ab90ab90ab90ab90ab, 128'h12341234123412341234123412341234, 128'h0000000000000000ffffffffffffffff);
   cn = 311; testcase_vvv(i16x8_narrow_i32x4_s,    128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 312; testcase_vvv(i16x8_narrow_i32x4_s,    128'h00000000000000000000000000000000, 128'h00000001000000010000000100000001, 128'h00000000000000000001000100010001);
   cn = 313; testcase_vvv(i16x8_narrow_i32x4_s,    128'h00000001000000010000000100000001, 128'h00000000000000000000000000000000, 128'h00010001000100010000000000000000);
   cn = 314; testcase_vvv(i16x8_narrow_i32x4_s,    128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h0000000000000000ffffffffffffffff);
   cn = 315; testcase_vvv(i16x8_narrow_i32x4_s,    128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffff0000000000000000);
   cn = 316; testcase_vvv(i16x8_narrow_i32x4_s,    128'h00000001000000010000000100000001, 128'hffffffffffffffffffffffffffffffff, 128'h0001000100010001ffffffffffffffff);
   cn = 317; testcase_vvv(i16x8_narrow_i32x4_s,    128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001, 128'hffffffffffffffff0001000100010001);
   cn = 318; testcase_vvv(i16x8_narrow_i32x4_s,    128'h00007ffe00007ffe00007ffe00007ffe, 128'h00007fff00007fff00007fff00007fff, 128'h7ffe7ffe7ffe7ffe7fff7fff7fff7fff);
   cn = 319; testcase_vvv(i16x8_narrow_i32x4_s,    128'h00007fff00007fff00007fff00007fff, 128'h00007ffe00007ffe00007ffe00007ffe, 128'h7fff7fff7fff7fff7ffe7ffe7ffe7ffe);
   cn = 320; testcase_vvv(i16x8_narrow_i32x4_s,    128'h00007fff00007fff00007fff00007fff, 128'h00007fff00007fff00007fff00007fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 321; testcase_vvv(i16x8_narrow_i32x4_s,    128'h00008000000080000000800000008000, 128'h00008000000080000000800000008000, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 322; testcase_vvv(i16x8_narrow_i32x4_s,    128'h00007fff00007fff00007fff00007fff, 128'h00008000000080000000800000008000, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 323; testcase_vvv(i16x8_narrow_i32x4_s,    128'h00008000000080000000800000008000, 128'h00007fff00007fff00007fff00007fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 324; testcase_vvv(i16x8_narrow_i32x4_s,    128'h00007fff00007fff00007fff00007fff, 128'h0000ffff0000ffff0000ffff0000ffff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 325; testcase_vvv(i16x8_narrow_i32x4_s,    128'h0000ffff0000ffff0000ffff0000ffff, 128'h00007fff00007fff00007fff00007fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 326; testcase_vvv(i16x8_narrow_i32x4_s,    128'hffff8001ffff8001ffff8001ffff8001, 128'hffff8000ffff8000ffff8000ffff8000, 128'h80018001800180018000800080008000);
   cn = 327; testcase_vvv(i16x8_narrow_i32x4_s,    128'hffff8000ffff8000ffff8000ffff8000, 128'hffff8001ffff8001ffff8001ffff8001, 128'h80008000800080008001800180018001);
   cn = 328; testcase_vvv(i16x8_narrow_i32x4_s,    128'hffff8000ffff8000ffff8000ffff8000, 128'hffff8000ffff8000ffff8000ffff8000, 128'h80008000800080008000800080008000);
   cn = 329; testcase_vvv(i16x8_narrow_i32x4_s,    128'hffff7fffffff7fffffff7fffffff7fff, 128'hffff7fffffff7fffffff7fffffff7fff, 128'h80008000800080008000800080008000);
   cn = 330; testcase_vvv(i16x8_narrow_i32x4_s,    128'hffff7fffffff7fffffff7fffffff7fff, 128'hffff8000ffff8000ffff8000ffff8000, 128'h80008000800080008000800080008000);
   cn = 331; testcase_vvv(i16x8_narrow_i32x4_s,    128'hffff8000ffff8000ffff8000ffff8000, 128'hffff7fffffff7fffffff7fffffff7fff, 128'h80008000800080008000800080008000);
   cn = 332; testcase_vvv(i16x8_narrow_i32x4_s,    128'hffff8000ffff8000ffff8000ffff8000, 128'h00007fff00007fff00007fff00007fff, 128'h80008000800080007fff7fff7fff7fff);
   cn = 333; testcase_vvv(i16x8_narrow_i32x4_s,    128'hffff8000ffff8000ffff8000ffff8000, 128'h00008000000080000000800000008000, 128'h80008000800080007fff7fff7fff7fff);
   cn = 334; testcase_vvv(i16x8_narrow_i32x4_s,    128'hffff7fffffff7fffffff7fffffff7fff, 128'h00007fff00007fff00007fff00007fff, 128'h80008000800080007fff7fff7fff7fff);
   cn = 335; testcase_vvv(i16x8_narrow_i32x4_s,    128'hffff8000ffff8000ffff8000ffff8000, 128'h0000ffff0000ffff0000ffff0000ffff, 128'h80008000800080007fff7fff7fff7fff);
   cn = 336; testcase_vvv(i16x8_narrow_i32x4_s,    128'hffff7fffffff7fffffff7fffffff7fff, 128'h00010000000100000001000000010000, 128'h80008000800080007fff7fff7fff7fff);
   cn = 337; testcase_vvv(i16x8_narrow_i32x4_s,    128'hf8000000f8000000f8000000f8000000, 128'hffffffffffffffffffffffffffffffff, 128'h8000800080008000ffffffffffffffff);
   cn = 338; testcase_vvv(i16x8_narrow_i32x4_s,    128'h075bcd15075bcd15075bcd15075bcd15, 128'h499602d2499602d2499602d2499602d2, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 339; testcase_vvv(i16x8_narrow_i32x4_s,    128'h90abcdef90abcdef90abcdef90abcdef, 128'h12345678123456781234567812345678, 128'h80008000800080007fff7fff7fff7fff);
   cn = 340; testcase_vvv(i16x8_narrow_i32x4_u,    128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 341; testcase_vvv(i16x8_narrow_i32x4_u,    128'h00000000000000000000000000000000, 128'h00000001000000010000000100000001, 128'h00000000000000000001000100010001);
   cn = 342; testcase_vvv(i16x8_narrow_i32x4_u,    128'h00000001000000010000000100000001, 128'h00000000000000000000000000000000, 128'h00010001000100010000000000000000);
   cn = 343; testcase_vvv(i16x8_narrow_i32x4_u,    128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 344; testcase_vvv(i16x8_narrow_i32x4_u,    128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 345; testcase_vvv(i16x8_narrow_i32x4_u,    128'h00000001000000010000000100000001, 128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010000000000000000);
   cn = 346; testcase_vvv(i16x8_narrow_i32x4_u,    128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001, 128'h00000000000000000001000100010001);
   cn = 347; testcase_vvv(i16x8_narrow_i32x4_u,    128'h0000fffe0000fffe0000fffe0000fffe, 128'h0000ffff0000ffff0000ffff0000ffff, 128'hfffefffefffefffeffffffffffffffff);
   cn = 348; testcase_vvv(i16x8_narrow_i32x4_u,    128'h0000ffff0000ffff0000ffff0000ffff, 128'h0000fffe0000fffe0000fffe0000fffe, 128'hfffffffffffffffffffefffefffefffe);
   cn = 349; testcase_vvv(i16x8_narrow_i32x4_u,    128'h0000ffff0000ffff0000ffff0000ffff, 128'h0000ffff0000ffff0000ffff0000ffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 350; testcase_vvv(i16x8_narrow_i32x4_u,    128'h00010000000100000001000000010000, 128'h00010000000100000001000000010000, 128'hffffffffffffffffffffffffffffffff);
   cn = 351; testcase_vvv(i16x8_narrow_i32x4_u,    128'h0000ffff0000ffff0000ffff0000ffff, 128'h00010000000100000001000000010000, 128'hffffffffffffffffffffffffffffffff);
   cn = 352; testcase_vvv(i16x8_narrow_i32x4_u,    128'h00010000000100000001000000010000, 128'h0000ffff0000ffff0000ffff0000ffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 353; testcase_vvv(i16x8_narrow_i32x4_u,    128'h00000000000000000000000000000000, 128'h0000ffff0000ffff0000ffff0000ffff, 128'h0000000000000000ffffffffffffffff);
   cn = 354; testcase_vvv(i16x8_narrow_i32x4_u,    128'h00000000000000000000000000000000, 128'h00010000000100000001000000010000, 128'h0000000000000000ffffffffffffffff);
   cn = 355; testcase_vvv(i16x8_narrow_i32x4_u,    128'hffffffffffffffffffffffffffffffff, 128'h0000ffff0000ffff0000ffff0000ffff, 128'h0000000000000000ffffffffffffffff);
   cn = 356; testcase_vvv(i16x8_narrow_i32x4_u,    128'hffffffffffffffffffffffffffffffff, 128'h00010000000100000001000000010000, 128'h0000000000000000ffffffffffffffff);
   cn = 357; testcase_vvv(i16x8_narrow_i32x4_u,    128'h80000000800000008000000080000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 358; testcase_vvv(i16x8_narrow_i32x4_u,    128'h075bcd15075bcd15075bcd15075bcd15, 128'h499602d2499602d2499602d2499602d2, 128'hffffffffffffffffffffffffffffffff);
   cn = 359; testcase_vvv(i16x8_narrow_i32x4_u,    128'h90abcdef90abcdef90abcdef90abcdef, 128'h12345678123456781234567812345678, 128'h0000000000000000ffffffffffffffff);
`else
   $display(" Testing  Unclipped Narrow");
   cn = 256; testcase_vv2v(i8x16_narrow_i16x8_s,   128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 257; testcase_vv2v(i8x16_narrow_i16x8_s,   128'h11112222333344445555666677778888, 128'h9999aaaabbbbccccddddeeeeffff0000, 128'h112233445566778899aabbccddeeff00, 128'h112233445566778899aabbccddeeff00);
   cn = 257; testcase_vv2v(i8x16_narrow_i16x8_s,   128'h00110022003300440055006600770088, 128'h009900aa00bb00cc00dd00ee00ffff00, 128'h112233445566778899aabbccddeeff00, 128'h000000000000000000000000000000ff);
   cn = 258; testcase_vv2v(i16x8_narrow_i32x4_s,   128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 259; testcase_vv2v(i16x8_narrow_i32x4_s,   128'h11111111222222223333333344444444, 128'h555555556666666677777777aaaaaaaa, 128'h1111222233334444555566667777aaaa, 128'h1111222233334444555566667777aaaa);
   cn = 260; testcase_vv2v(i16x8_narrow_i32x4_s,   128'h00001111000022220000333300004444, 128'h1111555511116666111177771111aaaa, 128'h1111222233334444555566667777aaaa, 128'h00000000000000001111111111111111);

`endif //ENABLE_NARROW
   $display(" Testing  add");
   cn = 360; testcase_vvv(i8x16_add,               128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 361; testcase_vvv(i8x16_add,               128'h00000000000000000000000000000000, 128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101);
   cn = 362; testcase_vvv(i8x16_add,               128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101, 128'h02020202020202020202020202020202);
   cn = 363; testcase_vvv(i8x16_add,               128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 364; testcase_vvv(i8x16_add,               128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 365; testcase_vvv(i8x16_add,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfefefefefefefefefefefefefefefefe);
   cn = 366; testcase_vvv(i8x16_add,               128'h3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f, 128'h40404040404040404040404040404040, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 367; testcase_vvv(i8x16_add,               128'h40404040404040404040404040404040, 128'h40404040404040404040404040404040, 128'h80808080808080808080808080808080);
   cn = 368; testcase_vvv(i8x16_add,               128'hc1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h81818181818181818181818181818181);
   cn = 369; testcase_vvv(i8x16_add,               128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h80808080808080808080808080808080);
   cn = 370; testcase_vvv(i8x16_add,               128'hbfbfbfbfbfbfbfbfbfbfbfbfbfbfbfbf, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 371; testcase_vvv(i8x16_add,               128'h7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d, 128'h01010101010101010101010101010101, 128'h7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e);
   cn = 372; testcase_vvv(i8x16_add,               128'h7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e, 128'h01010101010101010101010101010101, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 373; testcase_vvv(i8x16_add,               128'h80808080808080808080808080808080, 128'h01010101010101010101010101010101, 128'h81818181818181818181818181818181);
   cn = 374; testcase_vvv(i8x16_add,               128'h82828282828282828282828282828282, 128'hffffffffffffffffffffffffffffffff, 128'h81818181818181818181818181818181);
   cn = 375; testcase_vvv(i8x16_add,               128'h81818181818181818181818181818181, 128'hffffffffffffffffffffffffffffffff, 128'h80808080808080808080808080808080);
   cn = 376; testcase_vvv(i8x16_add,               128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 377; testcase_vvv(i8x16_add,               128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'hfefefefefefefefefefefefefefefefe);
   cn = 378; testcase_vvv(i8x16_add,               128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 379; testcase_vvv(i8x16_add,               128'h80808080808080808080808080808080, 128'h81818181818181818181818181818181, 128'h01010101010101010101010101010101);
   cn = 380; testcase_vvv(i8x16_add,               128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 381; testcase_vvv(i8x16_add,               128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101, 128'h00000000000000000000000000000000);
   cn = 382; testcase_vvv(i8x16_add,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfefefefefefefefefefefefefefefefe);
   cn = 383; testcase_vvv(i8x16_add,               128'hffffffffffffffffffffffffffffffff, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e);
   cn = 384; testcase_vvv(i8x16_add,               128'hffffffffffffffffffffffffffffffff, 128'h80808080808080808080808080808080, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 385; testcase_vvv(i8x16_add,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfefefefefefefefefefefefefefefefe);
   cn = 386; testcase_vvv(i8x16_add,               128'h3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f, 128'h40404040404040404040404040404040, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 387; testcase_vvv(i8x16_add,               128'h40404040404040404040404040404040, 128'h40404040404040404040404040404040, 128'h80808080808080808080808080808080);
   cn = 388; testcase_vvv(i8x16_add,               128'hc1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h81818181818181818181818181818181);
   cn = 389; testcase_vvv(i8x16_add,               128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h80808080808080808080808080808080);
   cn = 390; testcase_vvv(i8x16_add,               128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hbfbfbfbfbfbfbfbfbfbfbfbfbfbfbfbf, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 391; testcase_vvv(i8x16_add,               128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'hfefefefefefefefefefefefefefefefe);
   cn = 392; testcase_vvv(i8x16_add,               128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h01010101010101010101010101010101, 128'h80808080808080808080808080808080);
   cn = 393; testcase_vvv(i8x16_add,               128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 394; testcase_vvv(i8x16_add,               128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 395; testcase_vvv(i8x16_add,               128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 396; testcase_vvv(i8x16_add,               128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101, 128'h00000000000000000000000000000000);
   cn = 397; testcase_vvv(i8x16_add,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfefefefefefefefefefefefefefefefe);
   cn = 398; testcase_vvv(i8x16_add,               128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 399; testcase_vvv(i8x16_add,               128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 400; testcase_vvv(i8x16_add,               128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 401; testcase_vvv(i8x16_add,               128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 402; testcase_vvv(i8x16_add,               128'h000102030405060708090a0b0c0d0e0f, 128'h00fffefdfcfbfaf9f8f7f6f5f4f3f2f1, 128'h00000000000000000000000000000000);
   cn = 403; testcase_vvv(i8x16_add,               128'h000102030405060708090a0b0c0d0e0f, 128'h00020406080a0c0e10121416181a1c1e, 128'h000306090c0f1215181b1e2124272a2d);
   cn = 448; testcase_vvv(i16x8_add,               128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 449; testcase_vvv(i16x8_add,               128'h00000000000000000000000000000000, 128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001);
   cn = 450; testcase_vvv(i16x8_add,               128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001, 128'h00020002000200020002000200020002);
   cn = 451; testcase_vvv(i16x8_add,               128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 452; testcase_vvv(i16x8_add,               128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 453; testcase_vvv(i16x8_add,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfffefffefffefffefffefffefffefffe);
   cn = 454; testcase_vvv(i16x8_add,               128'h3fff3fff3fff3fff3fff3fff3fff3fff, 128'h40004000400040004000400040004000, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 455; testcase_vvv(i16x8_add,               128'h40004000400040004000400040004000, 128'h40004000400040004000400040004000, 128'h80008000800080008000800080008000);
   cn = 456; testcase_vvv(i16x8_add,               128'hc001c001c001c001c001c001c001c001, 128'hc000c000c000c000c000c000c000c000, 128'h80018001800180018001800180018001);
   cn = 457; testcase_vvv(i16x8_add,               128'hc000c000c000c000c000c000c000c000, 128'hc000c000c000c000c000c000c000c000, 128'h80008000800080008000800080008000);
   cn = 458; testcase_vvv(i16x8_add,               128'hbfffbfffbfffbfffbfffbfffbfffbfff, 128'hc000c000c000c000c000c000c000c000, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 459; testcase_vvv(i16x8_add,               128'h7ffd7ffd7ffd7ffd7ffd7ffd7ffd7ffd, 128'h00010001000100010001000100010001, 128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe);
   cn = 460; testcase_vvv(i16x8_add,               128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe, 128'h00010001000100010001000100010001, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 461; testcase_vvv(i16x8_add,               128'h80008000800080008000800080008000, 128'h00010001000100010001000100010001, 128'h80018001800180018001800180018001);
   cn = 462; testcase_vvv(i16x8_add,               128'h80028002800280028002800280028002, 128'hffffffffffffffffffffffffffffffff, 128'h80018001800180018001800180018001);
   cn = 463; testcase_vvv(i16x8_add,               128'h80018001800180018001800180018001, 128'hffffffffffffffffffffffffffffffff, 128'h80008000800080008000800080008000);
   cn = 464; testcase_vvv(i16x8_add,               128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 465; testcase_vvv(i16x8_add,               128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'hfffefffefffefffefffefffefffefffe);
   cn = 466; testcase_vvv(i16x8_add,               128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h00000000000000000000000000000000);
   cn = 467; testcase_vvv(i16x8_add,               128'h80008000800080008000800080008000, 128'h80018001800180018001800180018001, 128'h00010001000100010001000100010001);
   cn = 468; testcase_vvv(i16x8_add,               128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 469; testcase_vvv(i16x8_add,               128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'h00000000000000000000000000000000);
   cn = 470; testcase_vvv(i16x8_add,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfffefffefffefffefffefffefffefffe);
   cn = 471; testcase_vvv(i16x8_add,               128'hffffffffffffffffffffffffffffffff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe);
   cn = 472; testcase_vvv(i16x8_add,               128'hffffffffffffffffffffffffffffffff, 128'h80008000800080008000800080008000, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 473; testcase_vvv(i16x8_add,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfffefffefffefffefffefffefffefffe);
   cn = 474; testcase_vvv(i16x8_add,               128'h3fff3fff3fff3fff3fff3fff3fff3fff, 128'h40004000400040004000400040004000, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 475; testcase_vvv(i16x8_add,               128'h40004000400040004000400040004000, 128'h40004000400040004000400040004000, 128'h80008000800080008000800080008000);
   cn = 476; testcase_vvv(i16x8_add,               128'hc001c001c001c001c001c001c001c001, 128'hc000c000c000c000c000c000c000c000, 128'h80018001800180018001800180018001);
   cn = 477; testcase_vvv(i16x8_add,               128'hc000c000c000c000c000c000c000c000, 128'hc000c000c000c000c000c000c000c000, 128'h80008000800080008000800080008000);
   cn = 478; testcase_vvv(i16x8_add,               128'hc000c000c000c000c000c000c000c000, 128'hbfffbfffbfffbfffbfffbfffbfffbfff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 479; testcase_vvv(i16x8_add,               128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'hfffefffefffefffefffefffefffefffe);
   cn = 480; testcase_vvv(i16x8_add,               128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h00010001000100010001000100010001, 128'h80008000800080008000800080008000);
   cn = 481; testcase_vvv(i16x8_add,               128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 482; testcase_vvv(i16x8_add,               128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff);
   cn = 483; testcase_vvv(i16x8_add,               128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h00000000000000000000000000000000);
   cn = 484; testcase_vvv(i16x8_add,               128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'h00000000000000000000000000000000);
   cn = 485; testcase_vvv(i16x8_add,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfffefffefffefffefffefffefffefffe);
   cn = 486; testcase_vvv(i16x8_add,               128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff);
   cn = 487; testcase_vvv(i16x8_add,               128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 488; testcase_vvv(i16x8_add,               128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff);
   cn = 489; testcase_vvv(i16x8_add,               128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 490; testcase_vvv(i16x8_add,               128'h00000001000200030004000500060007, 128'h0000fffffffefffdfffcfffbfffafff9, 128'h00000000000000000000000000000000);
   cn = 491; testcase_vvv(i16x8_add,               128'h00000001000200030004000500060007, 128'h00000002000400060008000a000c000e, 128'h0000000300060009000c000f00120015);
   cn = 492; testcase_vvv(i16x8_add,               128'h30393039303930393039303930393039, 128'hddd5ddd5ddd5ddd5ddd5ddd5ddd5ddd5, 128'h0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e);
   cn = 493; testcase_vvv(i16x8_add,               128'h12341234123412341234123412341234, 128'h56785678567856785678567856785678, 128'h68ac68ac68ac68ac68ac68ac68ac68ac);

   cn = 777; testcase_vvv(i32x4_add,               128'h00000000000000000000000000000000, 128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001);
   cn = 778; testcase_vvv(i32x4_add,               128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001, 128'h00000002000000020000000200000002);
   cn = 779; testcase_vvv(i32x4_add,               128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 780; testcase_vvv(i32x4_add,               128'h00000001000000010000000100000001, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 781; testcase_vvv(i32x4_add,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfffffffefffffffefffffffefffffffe);
   cn = 782; testcase_vvv(i32x4_add,               128'h3fffffff3fffffff3fffffff3fffffff, 128'h40000000400000004000000040000000, 128'h7fffffff7fffffff7fffffff7fffffff);
   cn = 783; testcase_vvv(i32x4_add,               128'h40000000400000004000000040000000, 128'h40000000400000004000000040000000, 128'h80000000800000008000000080000000);
   cn = 784; testcase_vvv(i32x4_add,               128'hc0000001c0000001c0000001c0000001, 128'hc0000000c0000000c0000000c0000000, 128'h80000001800000018000000180000001);
   cn = 785; testcase_vvv(i32x4_add,               128'hc0000000c0000000c0000000c0000000, 128'hc0000000c0000000c0000000c0000000, 128'h80000000800000008000000080000000);
   cn = 786; testcase_vvv(i32x4_add,               128'hbfffffffbfffffffbfffffffbfffffff, 128'hc0000000c0000000c0000000c0000000, 128'h7fffffff7fffffff7fffffff7fffffff);
   cn = 787; testcase_vvv(i32x4_add,               128'h7ffffffd7ffffffd7ffffffd7ffffffd, 128'h00000001000000010000000100000001, 128'h7ffffffe7ffffffe7ffffffe7ffffffe);
   cn = 788; testcase_vvv(i32x4_add,               128'h7ffffffe7ffffffe7ffffffe7ffffffe, 128'h00000001000000010000000100000001, 128'h7fffffff7fffffff7fffffff7fffffff);
   cn = 789; testcase_vvv(i32x4_add,               128'h80000000800000008000000080000000, 128'h00000001000000010000000100000001, 128'h80000001800000018000000180000001);
   cn = 790; testcase_vvv(i32x4_add,               128'h80000002800000028000000280000002, 128'hffffffffffffffffffffffffffffffff, 128'h80000001800000018000000180000001);
   cn = 791; testcase_vvv(i32x4_add,               128'h80000001800000018000000180000001, 128'hffffffffffffffffffffffffffffffff, 128'h80000000800000008000000080000000);
   cn = 792; testcase_vvv(i32x4_add,               128'h80000000800000008000000080000000, 128'hffffffffffffffffffffffffffffffff, 128'h7fffffff7fffffff7fffffff7fffffff);
   cn = 793; testcase_vvv(i32x4_add,               128'h7fffffff7fffffff7fffffff7fffffff, 128'h7fffffff7fffffff7fffffff7fffffff, 128'hfffffffefffffffefffffffefffffffe);
   cn = 794; testcase_vvv(i32x4_add,               128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000, 128'h00000000000000000000000000000000);
   cn = 795; testcase_vvv(i32x4_add,               128'h80000000800000008000000080000000, 128'h80000001800000018000000180000001, 128'h00000001000000010000000100000001);
   cn = 796; testcase_vvv(i32x4_add,               128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 797; testcase_vvv(i32x4_add,               128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001, 128'h00000000000000000000000000000000);
   cn = 798; testcase_vvv(i32x4_add,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfffffffefffffffefffffffefffffffe);
   cn = 799; testcase_vvv(i32x4_add,               128'hffffffffffffffffffffffffffffffff, 128'h7fffffff7fffffff7fffffff7fffffff, 128'h7ffffffe7ffffffe7ffffffe7ffffffe);
   cn = 800; testcase_vvv(i32x4_add,               128'hffffffffffffffffffffffffffffffff, 128'h80000000800000008000000080000000, 128'h7fffffff7fffffff7fffffff7fffffff);
   cn = 801; testcase_vvv(i32x4_add,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfffffffefffffffefffffffefffffffe);
   cn = 802; testcase_vvv(i32x4_add,               128'h3fffffff3fffffff3fffffff3fffffff, 128'h40000000400000004000000040000000, 128'h7fffffff7fffffff7fffffff7fffffff);
   cn = 803; testcase_vvv(i32x4_add,               128'h40000000400000004000000040000000, 128'h40000000400000004000000040000000, 128'h80000000800000008000000080000000);
   cn = 804; testcase_vvv(i32x4_add,               128'hc0000001c0000001c0000001c0000001, 128'hc0000000c0000000c0000000c0000000, 128'h80000001800000018000000180000001);
   cn = 805; testcase_vvv(i32x4_add,               128'hc0000000c0000000c0000000c0000000, 128'hc0000000c0000000c0000000c0000000, 128'h80000000800000008000000080000000);
   cn = 806; testcase_vvv(i32x4_add,               128'hc0000000c0000000c0000000c0000000, 128'hbfffffffbfffffffbfffffffbfffffff, 128'h7fffffff7fffffff7fffffff7fffffff);
   cn = 807; testcase_vvv(i32x4_add,               128'h7fffffff7fffffff7fffffff7fffffff, 128'h7fffffff7fffffff7fffffff7fffffff, 128'hfffffffefffffffefffffffefffffffe);
   cn = 808; testcase_vvv(i32x4_add,               128'h7fffffff7fffffff7fffffff7fffffff, 128'h00000001000000010000000100000001, 128'h80000000800000008000000080000000);
   cn = 809; testcase_vvv(i32x4_add,               128'h80000000800000008000000080000000, 128'hffffffffffffffffffffffffffffffff, 128'h7fffffff7fffffff7fffffff7fffffff);
   cn = 810; testcase_vvv(i32x4_add,               128'h7fffffff7fffffff7fffffff7fffffff, 128'h80000000800000008000000080000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 811; testcase_vvv(i32x4_add,               128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000, 128'h00000000000000000000000000000000);
   cn = 812; testcase_vvv(i32x4_add,               128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001, 128'h00000000000000000000000000000000);
   cn = 813; testcase_vvv(i32x4_add,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfffffffefffffffefffffffefffffffe);
   cn = 815; testcase_vvv(i32x4_add,               128'h00000001000000010000000100000001, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 817; testcase_vvv(i32x4_add,               128'h00000001000000010000000100000001, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 818; testcase_vvv(i32x4_add,               128'h00000000000000010000000200000003, 128'h00000000fffffffffffffffefffffffd, 128'h00000000000000000000000000000000);
   cn = 819; testcase_vvv(i32x4_add,               128'h00000000000000010000000200000003, 128'h00000000000000020000000400000006, 128'h00000000000000030000000600000009);
   cn = 820; testcase_vvv(i32x4_add,               128'h499602d2499602d2499602d2499602d2, 128'h499602d2499602d2499602d2499602d2, 128'h932c05a4932c05a4932c05a4932c05a4);
   cn = 821; testcase_vvv(i32x4_add,               128'h12345678123456781234567812345678, 128'h90abcdef90abcdef90abcdef90abcdef, 128'ha2e02467a2e02467a2e02467a2e02467);
   cn = 822; testcase_vvv(i32x4_add,               128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);

   $display(" Testing  sub");
   cn = 404; testcase_vvv(i8x16_sub,               128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 405; testcase_vvv(i8x16_sub,               128'h00000000000000000000000000000000, 128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff);
   cn = 406; testcase_vvv(i8x16_sub,               128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101, 128'h00000000000000000000000000000000);
   cn = 407; testcase_vvv(i8x16_sub,               128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101);
   cn = 408; testcase_vvv(i8x16_sub,               128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff, 128'h02020202020202020202020202020202);
   cn = 409; testcase_vvv(i8x16_sub,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 410; testcase_vvv(i8x16_sub,               128'h3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f, 128'h40404040404040404040404040404040, 128'hffffffffffffffffffffffffffffffff);
   cn = 411; testcase_vvv(i8x16_sub,               128'h40404040404040404040404040404040, 128'h40404040404040404040404040404040, 128'h00000000000000000000000000000000);
   cn = 412; testcase_vvv(i8x16_sub,               128'hc1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h01010101010101010101010101010101);
   cn = 413; testcase_vvv(i8x16_sub,               128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h00000000000000000000000000000000);
   cn = 414; testcase_vvv(i8x16_sub,               128'hbfbfbfbfbfbfbfbfbfbfbfbfbfbfbfbf, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hffffffffffffffffffffffffffffffff);
   cn = 415; testcase_vvv(i8x16_sub,               128'h7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d, 128'h01010101010101010101010101010101, 128'h7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c);
   cn = 416; testcase_vvv(i8x16_sub,               128'h7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e, 128'h01010101010101010101010101010101, 128'h7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d);
   cn = 417; testcase_vvv(i8x16_sub,               128'h80808080808080808080808080808080, 128'h01010101010101010101010101010101, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 418; testcase_vvv(i8x16_sub,               128'h82828282828282828282828282828282, 128'hffffffffffffffffffffffffffffffff, 128'h83838383838383838383838383838383);
   cn = 419; testcase_vvv(i8x16_sub,               128'h81818181818181818181818181818181, 128'hffffffffffffffffffffffffffffffff, 128'h82828282828282828282828282828282);
   cn = 420; testcase_vvv(i8x16_sub,               128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff, 128'h81818181818181818181818181818181);
   cn = 421; testcase_vvv(i8x16_sub,               128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h00000000000000000000000000000000);
   cn = 422; testcase_vvv(i8x16_sub,               128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 423; testcase_vvv(i8x16_sub,               128'h80808080808080808080808080808080, 128'h81818181818181818181818181818181, 128'hffffffffffffffffffffffffffffffff);
   cn = 424; testcase_vvv(i8x16_sub,               128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 425; testcase_vvv(i8x16_sub,               128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101, 128'hfefefefefefefefefefefefefefefefe);
   cn = 426; testcase_vvv(i8x16_sub,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 427; testcase_vvv(i8x16_sub,               128'hffffffffffffffffffffffffffffffff, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h80808080808080808080808080808080);
   cn = 428; testcase_vvv(i8x16_sub,               128'hffffffffffffffffffffffffffffffff, 128'h80808080808080808080808080808080, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 429; testcase_vvv(i8x16_sub,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 430; testcase_vvv(i8x16_sub,               128'h3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f, 128'h40404040404040404040404040404040, 128'hffffffffffffffffffffffffffffffff);
   cn = 431; testcase_vvv(i8x16_sub,               128'h40404040404040404040404040404040, 128'h40404040404040404040404040404040, 128'h00000000000000000000000000000000);
   cn = 432; testcase_vvv(i8x16_sub,               128'hc1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h01010101010101010101010101010101);
   cn = 433; testcase_vvv(i8x16_sub,               128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h00000000000000000000000000000000);
   cn = 434; testcase_vvv(i8x16_sub,               128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hbfbfbfbfbfbfbfbfbfbfbfbfbfbfbfbf, 128'h01010101010101010101010101010101);
   cn = 435; testcase_vvv(i8x16_sub,               128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h00000000000000000000000000000000);
   cn = 436; testcase_vvv(i8x16_sub,               128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h01010101010101010101010101010101, 128'h7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e);
   cn = 437; testcase_vvv(i8x16_sub,               128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff, 128'h81818181818181818181818181818181);
   cn = 438; testcase_vvv(i8x16_sub,               128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 439; testcase_vvv(i8x16_sub,               128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 440; testcase_vvv(i8x16_sub,               128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101, 128'hfefefefefefefefefefefefefefefefe);
   cn = 441; testcase_vvv(i8x16_sub,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 442; testcase_vvv(i8x16_sub,               128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 443; testcase_vvv(i8x16_sub,               128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff, 128'h02020202020202020202020202020202);
   cn = 444; testcase_vvv(i8x16_sub,               128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 445; testcase_vvv(i8x16_sub,               128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff, 128'h02020202020202020202020202020202);
   cn = 446; testcase_vvv(i8x16_sub,               128'h000102030405060708090a0b0c0d0e0f, 128'h00fffefdfcfbfaf9f8f7f6f5f4f3f2f1, 128'h00020406080a0c0e10121416181a1c1e);
   cn = 447; testcase_vvv(i8x16_sub,               128'h000102030405060708090a0b0c0d0e0f, 128'h00020406080a0c0e10121416181a1c1e, 128'h00fffefdfcfbfaf9f8f7f6f5f4f3f2f1);

   cn = 494; testcase_vvv(i16x8_sub,               128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 495; testcase_vvv(i16x8_sub,               128'h00000000000000000000000000000000, 128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff);
   cn = 496; testcase_vvv(i16x8_sub,               128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001, 128'h00000000000000000000000000000000);
   cn = 497; testcase_vvv(i16x8_sub,               128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001);
   cn = 498; testcase_vvv(i16x8_sub,               128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff, 128'h00020002000200020002000200020002);
   cn = 499; testcase_vvv(i16x8_sub,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 500; testcase_vvv(i16x8_sub,               128'h3fff3fff3fff3fff3fff3fff3fff3fff, 128'h40004000400040004000400040004000, 128'hffffffffffffffffffffffffffffffff);
   cn = 501; testcase_vvv(i16x8_sub,               128'h40004000400040004000400040004000, 128'h40004000400040004000400040004000, 128'h00000000000000000000000000000000);
   cn = 502; testcase_vvv(i16x8_sub,               128'hc001c001c001c001c001c001c001c001, 128'hc000c000c000c000c000c000c000c000, 128'h00010001000100010001000100010001);
   cn = 503; testcase_vvv(i16x8_sub,               128'hc000c000c000c000c000c000c000c000, 128'hc000c000c000c000c000c000c000c000, 128'h00000000000000000000000000000000);
   cn = 504; testcase_vvv(i16x8_sub,               128'hbfffbfffbfffbfffbfffbfffbfffbfff, 128'hc000c000c000c000c000c000c000c000, 128'hffffffffffffffffffffffffffffffff);
   cn = 505; testcase_vvv(i16x8_sub,               128'h7ffd7ffd7ffd7ffd7ffd7ffd7ffd7ffd, 128'h00010001000100010001000100010001, 128'h7ffc7ffc7ffc7ffc7ffc7ffc7ffc7ffc);
   cn = 506; testcase_vvv(i16x8_sub,               128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe, 128'h00010001000100010001000100010001, 128'h7ffd7ffd7ffd7ffd7ffd7ffd7ffd7ffd);
   cn = 507; testcase_vvv(i16x8_sub,               128'h80008000800080008000800080008000, 128'h00010001000100010001000100010001, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 508; testcase_vvv(i16x8_sub,               128'h80028002800280028002800280028002, 128'hffffffffffffffffffffffffffffffff, 128'h80038003800380038003800380038003);
   cn = 509; testcase_vvv(i16x8_sub,               128'h80018001800180018001800180018001, 128'hffffffffffffffffffffffffffffffff, 128'h80028002800280028002800280028002);
   cn = 510; testcase_vvv(i16x8_sub,               128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff, 128'h80018001800180018001800180018001);
   cn = 511; testcase_vvv(i16x8_sub,               128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h00000000000000000000000000000000);
   cn = 512; testcase_vvv(i16x8_sub,               128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h00000000000000000000000000000000);
   cn = 513; testcase_vvv(i16x8_sub,               128'h80008000800080008000800080008000, 128'h80018001800180018001800180018001, 128'hffffffffffffffffffffffffffffffff);
   cn = 514; testcase_vvv(i16x8_sub,               128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 515; testcase_vvv(i16x8_sub,               128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'hfffefffefffefffefffefffefffefffe);
   cn = 516; testcase_vvv(i16x8_sub,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 517; testcase_vvv(i16x8_sub,               128'hffffffffffffffffffffffffffffffff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h80008000800080008000800080008000);
   cn = 518; testcase_vvv(i16x8_sub,               128'hffffffffffffffffffffffffffffffff, 128'h80008000800080008000800080008000, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 519; testcase_vvv(i16x8_sub,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 520; testcase_vvv(i16x8_sub,               128'h3fff3fff3fff3fff3fff3fff3fff3fff, 128'h40004000400040004000400040004000, 128'hffffffffffffffffffffffffffffffff);
   cn = 521; testcase_vvv(i16x8_sub,               128'h40004000400040004000400040004000, 128'h40004000400040004000400040004000, 128'h00000000000000000000000000000000);
   cn = 522; testcase_vvv(i16x8_sub,               128'hc001c001c001c001c001c001c001c001, 128'hc000c000c000c000c000c000c000c000, 128'h00010001000100010001000100010001);
   cn = 523; testcase_vvv(i16x8_sub,               128'hc000c000c000c000c000c000c000c000, 128'hc000c000c000c000c000c000c000c000, 128'h00000000000000000000000000000000);
   cn = 524; testcase_vvv(i16x8_sub,               128'hc000c000c000c000c000c000c000c000, 128'hbfffbfffbfffbfffbfffbfffbfffbfff, 128'h00010001000100010001000100010001);
   cn = 525; testcase_vvv(i16x8_sub,               128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h00000000000000000000000000000000);
   cn = 526; testcase_vvv(i16x8_sub,               128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h00010001000100010001000100010001, 128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe);
   cn = 527; testcase_vvv(i16x8_sub,               128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff, 128'h80018001800180018001800180018001);
   cn = 528; testcase_vvv(i16x8_sub,               128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff);
   cn = 529; testcase_vvv(i16x8_sub,               128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h00000000000000000000000000000000);
   cn = 530; testcase_vvv(i16x8_sub,               128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'hfffefffefffefffefffefffefffefffe);
   cn = 531; testcase_vvv(i16x8_sub,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 532; testcase_vvv(i16x8_sub,               128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff);
   cn = 533; testcase_vvv(i16x8_sub,               128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff, 128'h00020002000200020002000200020002);
   cn = 534; testcase_vvv(i16x8_sub,               128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff);
   cn = 535; testcase_vvv(i16x8_sub,               128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff, 128'h00020002000200020002000200020002);
   cn = 536; testcase_vvv(i16x8_sub,               128'h00000001000200030004000500060007, 128'h0000fffffffefffdfffcfffbfffafff9, 128'h00000002000400060008000a000c000e);
   cn = 537; testcase_vvv(i16x8_sub,               128'h00000001000200030004000500060007, 128'h00000002000400060008000a000c000e, 128'h0000fffffffefffdfffcfffbfffafff9);
   cn = 538; testcase_vvv(i16x8_sub,               128'hddd5ddd5ddd5ddd5ddd5ddd5ddd5ddd5, 128'h30393039303930393039303930393039, 128'had9cad9cad9cad9cad9cad9cad9cad9c);
   cn = 539; testcase_vvv(i16x8_sub,               128'h56785678567856785678567856785678, 128'h12341234123412341234123412341234, 128'h44444444444444444444444444444444);

   cn = 824; testcase_vvv(i32x4_sub,               128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 825; testcase_vvv(i32x4_sub,               128'h00000000000000000000000000000000, 128'h00000001000000010000000100000001, 128'hffffffffffffffffffffffffffffffff);
   cn = 826; testcase_vvv(i32x4_sub,               128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001, 128'h00000000000000000000000000000000);
   cn = 827; testcase_vvv(i32x4_sub,               128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001);
   cn = 828; testcase_vvv(i32x4_sub,               128'h00000001000000010000000100000001, 128'hffffffffffffffffffffffffffffffff, 128'h00000002000000020000000200000002);
   cn = 829; testcase_vvv(i32x4_sub,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 830; testcase_vvv(i32x4_sub,               128'h3fffffff3fffffff3fffffff3fffffff, 128'h40000000400000004000000040000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 831; testcase_vvv(i32x4_sub,               128'h40000000400000004000000040000000, 128'h40000000400000004000000040000000, 128'h00000000000000000000000000000000);
   cn = 832; testcase_vvv(i32x4_sub,               128'hc0000001c0000001c0000001c0000001, 128'hc0000000c0000000c0000000c0000000, 128'h00000001000000010000000100000001);
   cn = 833; testcase_vvv(i32x4_sub,               128'hc0000000c0000000c0000000c0000000, 128'hc0000000c0000000c0000000c0000000, 128'h00000000000000000000000000000000);
   cn = 834; testcase_vvv(i32x4_sub,               128'hbfffffffbfffffffbfffffffbfffffff, 128'hc0000000c0000000c0000000c0000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 835; testcase_vvv(i32x4_sub,               128'h7ffffffd7ffffffd7ffffffd7ffffffd, 128'h00000001000000010000000100000001, 128'h7ffffffc7ffffffc7ffffffc7ffffffc);
   cn = 836; testcase_vvv(i32x4_sub,               128'h7ffffffe7ffffffe7ffffffe7ffffffe, 128'h00000001000000010000000100000001, 128'h7ffffffd7ffffffd7ffffffd7ffffffd);
   cn = 837; testcase_vvv(i32x4_sub,               128'h80000000800000008000000080000000, 128'h00000001000000010000000100000001, 128'h7fffffff7fffffff7fffffff7fffffff);
   cn = 838; testcase_vvv(i32x4_sub,               128'h80000002800000028000000280000002, 128'hffffffffffffffffffffffffffffffff, 128'h80000003800000038000000380000003);
   cn = 839; testcase_vvv(i32x4_sub,               128'h80000001800000018000000180000001, 128'hffffffffffffffffffffffffffffffff, 128'h80000002800000028000000280000002);
   cn = 840; testcase_vvv(i32x4_sub,               128'h80000000800000008000000080000000, 128'hffffffffffffffffffffffffffffffff, 128'h80000001800000018000000180000001);
   cn = 841; testcase_vvv(i32x4_sub,               128'h7fffffff7fffffff7fffffff7fffffff, 128'h7fffffff7fffffff7fffffff7fffffff, 128'h00000000000000000000000000000000);
   cn = 842; testcase_vvv(i32x4_sub,               128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000, 128'h00000000000000000000000000000000);
   cn = 843; testcase_vvv(i32x4_sub,               128'h80000000800000008000000080000000, 128'h80000001800000018000000180000001, 128'hffffffffffffffffffffffffffffffff);
   cn = 844; testcase_vvv(i32x4_sub,               128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 845; testcase_vvv(i32x4_sub,               128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001, 128'hfffffffefffffffefffffffefffffffe);
   cn = 846; testcase_vvv(i32x4_sub,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 847; testcase_vvv(i32x4_sub,               128'hffffffffffffffffffffffffffffffff, 128'h7fffffff7fffffff7fffffff7fffffff, 128'h80000000800000008000000080000000);
   cn = 848; testcase_vvv(i32x4_sub,               128'hffffffffffffffffffffffffffffffff, 128'h80000000800000008000000080000000, 128'h7fffffff7fffffff7fffffff7fffffff);
   cn = 849; testcase_vvv(i32x4_sub,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 850; testcase_vvv(i32x4_sub,               128'h3fffffff3fffffff3fffffff3fffffff, 128'h40000000400000004000000040000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 851; testcase_vvv(i32x4_sub,               128'h40000000400000004000000040000000, 128'h40000000400000004000000040000000, 128'h00000000000000000000000000000000);
   cn = 852; testcase_vvv(i32x4_sub,               128'hc0000001c0000001c0000001c0000001, 128'hc0000000c0000000c0000000c0000000, 128'h00000001000000010000000100000001);
   cn = 853; testcase_vvv(i32x4_sub,               128'hc0000000c0000000c0000000c0000000, 128'hc0000000c0000000c0000000c0000000, 128'h00000000000000000000000000000000);
   cn = 854; testcase_vvv(i32x4_sub,               128'hc0000000c0000000c0000000c0000000, 128'hbfffffffbfffffffbfffffffbfffffff, 128'h00000001000000010000000100000001);
   cn = 855; testcase_vvv(i32x4_sub,               128'h7fffffff7fffffff7fffffff7fffffff, 128'h7fffffff7fffffff7fffffff7fffffff, 128'h00000000000000000000000000000000);
   cn = 856; testcase_vvv(i32x4_sub,               128'h7fffffff7fffffff7fffffff7fffffff, 128'h00000001000000010000000100000001, 128'h7ffffffe7ffffffe7ffffffe7ffffffe);
   cn = 857; testcase_vvv(i32x4_sub,               128'h80000000800000008000000080000000, 128'hffffffffffffffffffffffffffffffff, 128'h80000001800000018000000180000001);
   cn = 858; testcase_vvv(i32x4_sub,               128'h7fffffff7fffffff7fffffff7fffffff, 128'h80000000800000008000000080000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 859; testcase_vvv(i32x4_sub,               128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000, 128'h00000000000000000000000000000000);
   cn = 860; testcase_vvv(i32x4_sub,               128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001, 128'hfffffffefffffffefffffffefffffffe);
   cn = 861; testcase_vvv(i32x4_sub,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 863; testcase_vvv(i32x4_sub,               128'h00000001000000010000000100000001, 128'hffffffffffffffffffffffffffffffff, 128'h00000002000000020000000200000002);
   cn = 865; testcase_vvv(i32x4_sub,               128'h00000001000000010000000100000001, 128'hffffffffffffffffffffffffffffffff, 128'h00000002000000020000000200000002);
   cn = 866; testcase_vvv(i32x4_sub,               128'h00000000000000010000000200000003, 128'h00000000fffffffffffffffefffffffd, 128'h00000000000000020000000400000006);
   cn = 867; testcase_vvv(i32x4_sub,               128'h00000000000000010000000200000003, 128'h00000000000000020000000400000006, 128'h00000000fffffffffffffffefffffffd);
   cn = 868; testcase_vvv(i32x4_sub,               128'hbf9a69d2bf9a69d2bf9a69d2bf9a69d2, 128'h499602d2499602d2499602d2499602d2, 128'h76046700760467007604670076046700);
   cn = 869; testcase_vvv(i32x4_sub,               128'h90abcdef90abcdef90abcdef90abcdef, 128'h12345678123456781234567812345678, 128'h7e7777777e7777777e7777777e777777);

   $display(" Testing  mul");
   cn = 540; testcase_vvv(i16x8_mul,               128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 541; testcase_vvv(i16x8_mul,               128'h00000000000000000000000000000000, 128'h00010001000100010001000100010001, 128'h00000000000000000000000000000000);
   cn = 542; testcase_vvv(i16x8_mul,               128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001);
   cn = 543; testcase_vvv(i16x8_mul,               128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 544; testcase_vvv(i16x8_mul,               128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 545; testcase_vvv(i16x8_mul,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001);
   cn = 546; testcase_vvv(i16x8_mul,               128'h3fff3fff3fff3fff3fff3fff3fff3fff, 128'h40004000400040004000400040004000, 128'hc000c000c000c000c000c000c000c000);
   cn = 547; testcase_vvv(i16x8_mul,               128'h40004000400040004000400040004000, 128'h40004000400040004000400040004000, 128'h00000000000000000000000000000000);
   cn = 548; testcase_vvv(i16x8_mul,               128'hc001c001c001c001c001c001c001c001, 128'hc000c000c000c000c000c000c000c000, 128'hc000c000c000c000c000c000c000c000);
   cn = 549; testcase_vvv(i16x8_mul,               128'hc000c000c000c000c000c000c000c000, 128'hc000c000c000c000c000c000c000c000, 128'h00000000000000000000000000000000);
   cn = 550; testcase_vvv(i16x8_mul,               128'hbfffbfffbfffbfffbfffbfffbfffbfff, 128'hc000c000c000c000c000c000c000c000, 128'h40004000400040004000400040004000);
   cn = 551; testcase_vvv(i16x8_mul,               128'h7ffd7ffd7ffd7ffd7ffd7ffd7ffd7ffd, 128'h00010001000100010001000100010001, 128'h7ffd7ffd7ffd7ffd7ffd7ffd7ffd7ffd);
   cn = 552; testcase_vvv(i16x8_mul,               128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe, 128'h00010001000100010001000100010001, 128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe);
   cn = 553; testcase_vvv(i16x8_mul,               128'h80008000800080008000800080008000, 128'h00010001000100010001000100010001, 128'h80008000800080008000800080008000);
   cn = 554; testcase_vvv(i16x8_mul,               128'h80028002800280028002800280028002, 128'hffffffffffffffffffffffffffffffff, 128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe);
   cn = 555; testcase_vvv(i16x8_mul,               128'h80018001800180018001800180018001, 128'hffffffffffffffffffffffffffffffff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 556; testcase_vvv(i16x8_mul,               128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff, 128'h80008000800080008000800080008000);
   cn = 557; testcase_vvv(i16x8_mul,               128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h00010001000100010001000100010001);
   cn = 558; testcase_vvv(i16x8_mul,               128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h00000000000000000000000000000000);
   cn = 559; testcase_vvv(i16x8_mul,               128'h80008000800080008000800080008000, 128'h80018001800180018001800180018001, 128'h80008000800080008000800080008000);
   cn = 560; testcase_vvv(i16x8_mul,               128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 561; testcase_vvv(i16x8_mul,               128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff);
   cn = 562; testcase_vvv(i16x8_mul,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001);
   cn = 563; testcase_vvv(i16x8_mul,               128'hffffffffffffffffffffffffffffffff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h80018001800180018001800180018001);
   cn = 564; testcase_vvv(i16x8_mul,               128'hffffffffffffffffffffffffffffffff, 128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 565; testcase_vvv(i16x8_mul,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001);
   cn = 566; testcase_vvv(i16x8_mul,               128'h3fff3fff3fff3fff3fff3fff3fff3fff, 128'h40004000400040004000400040004000, 128'hc000c000c000c000c000c000c000c000);
   cn = 567; testcase_vvv(i16x8_mul,               128'h40004000400040004000400040004000, 128'h40004000400040004000400040004000, 128'h00000000000000000000000000000000);
   cn = 568; testcase_vvv(i16x8_mul,               128'hc001c001c001c001c001c001c001c001, 128'hc000c000c000c000c000c000c000c000, 128'hc000c000c000c000c000c000c000c000);
   cn = 569; testcase_vvv(i16x8_mul,               128'hc000c000c000c000c000c000c000c000, 128'hc000c000c000c000c000c000c000c000, 128'h00000000000000000000000000000000);
   cn = 570; testcase_vvv(i16x8_mul,               128'hc000c000c000c000c000c000c000c000, 128'hbfffbfffbfffbfffbfffbfffbfffbfff, 128'h40004000400040004000400040004000);
   cn = 571; testcase_vvv(i16x8_mul,               128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h00010001000100010001000100010001);
   cn = 572; testcase_vvv(i16x8_mul,               128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h00010001000100010001000100010001, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 573; testcase_vvv(i16x8_mul,               128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff, 128'h80008000800080008000800080008000);
   cn = 574; testcase_vvv(i16x8_mul,               128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 575; testcase_vvv(i16x8_mul,               128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h00000000000000000000000000000000);
   cn = 576; testcase_vvv(i16x8_mul,               128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff);
   cn = 577; testcase_vvv(i16x8_mul,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001);
   cn = 578; testcase_vvv(i16x8_mul,               128'h10001000100010001000100010001000, 128'h10101010101010101010101010101010, 128'h00000000000000000000000000000000);
   cn = 579; testcase_vvv(i16x8_mul,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001);
   cn = 580; testcase_vvv(i16x8_mul,               128'h80008000800080008000800080008000, 128'h00020002000200020002000200020002, 128'h00000000000000000000000000000000);
   cn = 581; testcase_vvv(i16x8_mul,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001);
   cn = 582; testcase_vvv(i16x8_mul,               128'h00000001000200030004000500060007, 128'h0000fffffffefffdfffcfffbfffafff9, 128'h0000fffffffcfff7fff0ffe7ffdcffcf);
   cn = 583; testcase_vvv(i16x8_mul,               128'h00000001000200030004000500060007, 128'h00000002000400060008000a000c000e, 128'h00000002000800120020003200480062);
   cn = 584; testcase_vvv(i16x8_mul,               128'h30393039303930393039303930393039, 128'hddd5ddd5ddd5ddd5ddd5ddd5ddd5ddd5, 128'h546d546d546d546d546d546d546d546d);
   cn = 585; testcase_vvv(i16x8_mul,               128'h12341234123412341234123412341234, 128'hcdefcdefcdefcdefcdefcdefcdefcdef, 128'ha28ca28ca28ca28ca28ca28ca28ca28c);

   cn = 870; testcase_vvv(i32x4_mul,               128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 871; testcase_vvv(i32x4_mul,               128'h00000000000000000000000000000000, 128'h00000001000000010000000100000001, 128'h00000000000000000000000000000000);
   cn = 872; testcase_vvv(i32x4_mul,               128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001);
   cn = 873; testcase_vvv(i32x4_mul,               128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 874; testcase_vvv(i32x4_mul,               128'h00000001000000010000000100000001, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 875; testcase_vvv(i32x4_mul,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001);
   cn = 876; testcase_vvv(i32x4_mul,               128'h3fffffff3fffffff3fffffff3fffffff, 128'h40000000400000004000000040000000, 128'hc0000000c0000000c0000000c0000000);
   cn = 877; testcase_vvv(i32x4_mul,               128'h40000000400000004000000040000000, 128'h40000000400000004000000040000000, 128'h00000000000000000000000000000000);
   cn = 878; testcase_vvv(i32x4_mul,               128'hc0000001c0000001c0000001c0000001, 128'hc0000000c0000000c0000000c0000000, 128'hc0000000c0000000c0000000c0000000);
   cn = 879; testcase_vvv(i32x4_mul,               128'hc0000000c0000000c0000000c0000000, 128'hc0000000c0000000c0000000c0000000, 128'h00000000000000000000000000000000);
   cn = 880; testcase_vvv(i32x4_mul,               128'hbfffffffbfffffffbfffffffbfffffff, 128'hc0000000c0000000c0000000c0000000, 128'h40000000400000004000000040000000);
   cn = 881; testcase_vvv(i32x4_mul,               128'h7ffffffd7ffffffd7ffffffd7ffffffd, 128'h00000001000000010000000100000001, 128'h7ffffffd7ffffffd7ffffffd7ffffffd);
   cn = 882; testcase_vvv(i32x4_mul,               128'h7ffffffe7ffffffe7ffffffe7ffffffe, 128'h00000001000000010000000100000001, 128'h7ffffffe7ffffffe7ffffffe7ffffffe);
   cn = 883; testcase_vvv(i32x4_mul,               128'h80000000800000008000000080000000, 128'h00000001000000010000000100000001, 128'h80000000800000008000000080000000);
   cn = 884; testcase_vvv(i32x4_mul,               128'h80000002800000028000000280000002, 128'hffffffffffffffffffffffffffffffff, 128'h7ffffffe7ffffffe7ffffffe7ffffffe);
   cn = 885; testcase_vvv(i32x4_mul,               128'h80000001800000018000000180000001, 128'hffffffffffffffffffffffffffffffff, 128'h7fffffff7fffffff7fffffff7fffffff);
   cn = 886; testcase_vvv(i32x4_mul,               128'h80000000800000008000000080000000, 128'hffffffffffffffffffffffffffffffff, 128'h80000000800000008000000080000000);
   cn = 887; testcase_vvv(i32x4_mul,               128'h7fffffff7fffffff7fffffff7fffffff, 128'h7fffffff7fffffff7fffffff7fffffff, 128'h00000001000000010000000100000001);
   cn = 888; testcase_vvv(i32x4_mul,               128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000, 128'h00000000000000000000000000000000);
   cn = 889; testcase_vvv(i32x4_mul,               128'h80000000800000008000000080000000, 128'h80000001800000018000000180000001, 128'h80000000800000008000000080000000);
   cn = 890; testcase_vvv(i32x4_mul,               128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 891; testcase_vvv(i32x4_mul,               128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001, 128'hffffffffffffffffffffffffffffffff);
   cn = 892; testcase_vvv(i32x4_mul,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001);
   cn = 893; testcase_vvv(i32x4_mul,               128'hffffffffffffffffffffffffffffffff, 128'h7fffffff7fffffff7fffffff7fffffff, 128'h80000001800000018000000180000001);
   cn = 894; testcase_vvv(i32x4_mul,               128'hffffffffffffffffffffffffffffffff, 128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000);
   cn = 895; testcase_vvv(i32x4_mul,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001);
   cn = 896; testcase_vvv(i32x4_mul,               128'h3fffffff3fffffff3fffffff3fffffff, 128'h40000000400000004000000040000000, 128'hc0000000c0000000c0000000c0000000);
   cn = 897; testcase_vvv(i32x4_mul,               128'h40000000400000004000000040000000, 128'h40000000400000004000000040000000, 128'h00000000000000000000000000000000);
   cn = 898; testcase_vvv(i32x4_mul,               128'hc0000001c0000001c0000001c0000001, 128'hc0000000c0000000c0000000c0000000, 128'hc0000000c0000000c0000000c0000000);
   cn = 899; testcase_vvv(i32x4_mul,               128'hc0000000c0000000c0000000c0000000, 128'hc0000000c0000000c0000000c0000000, 128'h00000000000000000000000000000000);
   cn = 900; testcase_vvv(i32x4_mul,               128'hc0000000c0000000c0000000c0000000, 128'hbfffffffbfffffffbfffffffbfffffff, 128'h40000000400000004000000040000000);
   cn = 901; testcase_vvv(i32x4_mul,               128'h7fffffff7fffffff7fffffff7fffffff, 128'h7fffffff7fffffff7fffffff7fffffff, 128'h00000001000000010000000100000001);
   cn = 902; testcase_vvv(i32x4_mul,               128'h7fffffff7fffffff7fffffff7fffffff, 128'h00000001000000010000000100000001, 128'h7fffffff7fffffff7fffffff7fffffff);
   cn = 903; testcase_vvv(i32x4_mul,               128'h80000000800000008000000080000000, 128'hffffffffffffffffffffffffffffffff, 128'h80000000800000008000000080000000);
   cn = 904; testcase_vvv(i32x4_mul,               128'h7fffffff7fffffff7fffffff7fffffff, 128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000);
   cn = 905; testcase_vvv(i32x4_mul,               128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000, 128'h00000000000000000000000000000000);
   cn = 906; testcase_vvv(i32x4_mul,               128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001, 128'hffffffffffffffffffffffffffffffff);
   cn = 907; testcase_vvv(i32x4_mul,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001);
   cn = 908; testcase_vvv(i32x4_mul,               128'h10000000100000001000000010000000, 128'h10101010101010101010101010101010, 128'h00000000000000000000000000000000);
   cn = 909; testcase_vvv(i32x4_mul,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001);
   cn = 910; testcase_vvv(i32x4_mul,               128'h80000000800000008000000080000000, 128'h00000002000000020000000200000002, 128'h00000000000000000000000000000000);
   cn = 911; testcase_vvv(i32x4_mul,               128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001);
   cn = 912; testcase_vvv(i32x4_mul,               128'h00000000000000010000000200000003, 128'h00000000fffffffffffffffefffffffd, 128'h00000000fffffffffffffffcfffffff7);
   cn = 913; testcase_vvv(i32x4_mul,               128'h00000000000000010000000200000003, 128'h00000000000000020000000400000006, 128'h00000000000000020000000800000012);
   cn = 914; testcase_vvv(i32x4_mul,               128'h075bcd15075bcd15075bcd15075bcd15, 128'h3ade68b13ade68b13ade68b13ade68b1, 128'hfbff5385fbff5385fbff5385fbff5385);
   cn = 915; testcase_vvv(i32x4_mul,               128'h12345678123456781234567812345678, 128'h90abcdef90abcdef90abcdef90abcdef, 128'h2a42d2082a42d2082a42d2082a42d208);

`ifdef   ENABLE_NEGABSSAT
   $display(" Testing  neg");
   cn = 586; testcase_vv(i8x16_neg,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 587; testcase_vv(i8x16_neg,                128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff);
   cn = 588; testcase_vv(i8x16_neg,                128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101);
   cn = 589; testcase_vv(i8x16_neg,                128'h7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e, 128'h82828282828282828282828282828282);
   cn = 580; testcase_vv(i8x16_neg,                128'h81818181818181818181818181818181, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 581; testcase_vv(i8x16_neg,                128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 582; testcase_vv(i8x16_neg,                128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h81818181818181818181818181818181);
   cn = 583; testcase_vv(i8x16_neg,                128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101);
   cn = 584; testcase_vv(i8x16_neg,                128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff);
   cn = 585; testcase_vv(i8x16_neg,                128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101);
   cn = 586; testcase_vv(i8x16_neg,                128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 587; testcase_vv(i8x16_neg,                128'h81818181818181818181818181818181, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 588; testcase_vv(i8x16_neg,                128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h81818181818181818181818181818181);
   cn = 589; testcase_vv(i8x16_neg,                128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 590; testcase_vv(i8x16_neg,                128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101);
   cn = 591; testcase_vv(i16x8_neg,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 592; testcase_vv(i16x8_neg,                128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff);
   cn = 593; testcase_vv(i16x8_neg,                128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001);
   cn = 594; testcase_vv(i16x8_neg,                128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe, 128'h80028002800280028002800280028002);
   cn = 595; testcase_vv(i16x8_neg,                128'h80018001800180018001800180018001, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 596; testcase_vv(i16x8_neg,                128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 597; testcase_vv(i16x8_neg,                128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h80018001800180018001800180018001);
   cn = 598; testcase_vv(i16x8_neg,                128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001);
   cn = 599; testcase_vv(i16x8_neg,                128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff);
   cn = 600; testcase_vv(i16x8_neg,                128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001);
   cn = 601; testcase_vv(i16x8_neg,                128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 602; testcase_vv(i16x8_neg,                128'h80018001800180018001800180018001, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 603; testcase_vv(i16x8_neg,                128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h80018001800180018001800180018001);
   cn = 604; testcase_vv(i16x8_neg,                128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 605; testcase_vv(i16x8_neg,                128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001);
   cn = 606; testcase_vv(i32x4_neg,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 607; testcase_vv(i32x4_neg,                128'h00000001000000010000000100000001, 128'hffffffffffffffffffffffffffffffff);
   cn = 608; testcase_vv(i32x4_neg,                128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001);
   cn = 609; testcase_vv(i32x4_neg,                128'h7ffffffe7ffffffe7ffffffe7ffffffe, 128'h80000002800000028000000280000002);
   cn = 610; testcase_vv(i32x4_neg,                128'h80000001800000018000000180000001, 128'h7fffffff7fffffff7fffffff7fffffff);
   cn = 611; testcase_vv(i32x4_neg,                128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000);
   cn = 612; testcase_vv(i32x4_neg,                128'h7fffffff7fffffff7fffffff7fffffff, 128'h80000001800000018000000180000001);
   cn = 613; testcase_vv(i32x4_neg,                128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001);
   cn = 614; testcase_vv(i32x4_neg,                128'h00000001000000010000000100000001, 128'hffffffffffffffffffffffffffffffff);
   cn = 615; testcase_vv(i32x4_neg,                128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001);
   cn = 616; testcase_vv(i32x4_neg,                128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000);
   cn = 617; testcase_vv(i32x4_neg,                128'h80000001800000018000000180000001, 128'h7fffffff7fffffff7fffffff7fffffff);
   cn = 618; testcase_vv(i32x4_neg,                128'h7fffffff7fffffff7fffffff7fffffff, 128'h80000001800000018000000180000001);
   cn = 619; testcase_vv(i32x4_neg,                128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000);
   cn = 620; testcase_vv(i32x4_neg,                128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001);
`endif //ENABLE_NEGABSSAT

   $display(" Testing  min");
   cn = 621; testcase_vvv(i8x16_min_s,             128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 622; testcase_vvv(i8x16_min_s,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 623; testcase_vvv(i8x16_min_s,             128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h00000000ffffffffffffffffffffffff);
   cn = 624; testcase_vvv(i8x16_min_s,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 625; testcase_vvv(i8x16_min_s,             128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101);
   cn = 626; testcase_vvv(i8x16_min_s,             128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff);
   cn = 627; testcase_vvv(i8x16_min_s,             128'hffffffffffffffffffffffffffffffff, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 628; testcase_vvv(i8x16_min_s,             128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 629; testcase_vvv(i8x16_min_s,             128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 630; testcase_vvv(i8x16_min_s,             128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b, 128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b, 128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b);
   cn = 631; testcase_vvv(i8x16_min_s,             128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 632; testcase_vvv(i8x16_min_u,             128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 633; testcase_vvv(i8x16_min_u,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 634; testcase_vvv(i8x16_min_u,             128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h000000000000000000000000ffffffff);
   cn = 635; testcase_vvv(i8x16_min_u,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 636; testcase_vvv(i8x16_min_u,             128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101);
   cn = 637; testcase_vvv(i8x16_min_u,             128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101);
   cn = 638; testcase_vvv(i8x16_min_u,             128'hffffffffffffffffffffffffffffffff, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 639; testcase_vvv(i8x16_min_u,             128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 640; testcase_vvv(i8x16_min_u,             128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 641; testcase_vvv(i8x16_min_u,             128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b, 128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b, 128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b);
   cn = 642; testcase_vvv(i8x16_min_u,             128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 643; testcase_vvv(i16x8_min_s,             128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 644; testcase_vvv(i16x8_min_s,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 645; testcase_vvv(i16x8_min_s,             128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h00000000ffffffffffffffffffffffff);
   cn = 646; testcase_vvv(i16x8_min_s,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 647; testcase_vvv(i16x8_min_s,             128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001);
   cn = 648; testcase_vvv(i16x8_min_s,             128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff);
   cn = 649; testcase_vvv(i16x8_min_s,             128'hffffffffffffffffffffffffffffffff, 128'h00800080008000800080008000800080, 128'hffffffffffffffffffffffffffffffff);
   cn = 650; testcase_vvv(i16x8_min_s,             128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 651; testcase_vvv(i16x8_min_s,             128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 652; testcase_vvv(i16x8_min_s,             128'h007b007b007b007b007b007b007b007b, 128'h007b007b007b007b007b007b007b007b, 128'h007b007b007b007b007b007b007b007b);
   cn = 653; testcase_vvv(i16x8_min_s,             128'h00800080008000800080008000800080, 128'h00800080008000800080008000800080, 128'h00800080008000800080008000800080);
   cn = 654; testcase_vvv(i16x8_min_u,             128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 655; testcase_vvv(i16x8_min_u,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 656; testcase_vvv(i16x8_min_u,             128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h000000000000000000000000ffffffff);
   cn = 657; testcase_vvv(i16x8_min_u,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 658; testcase_vvv(i16x8_min_u,             128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001);
   cn = 659; testcase_vvv(i16x8_min_u,             128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001);
   cn = 660; testcase_vvv(i16x8_min_u,             128'hffffffffffffffffffffffffffffffff, 128'h00800080008000800080008000800080, 128'h00800080008000800080008000800080);
   cn = 661; testcase_vvv(i16x8_min_u,             128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 662; testcase_vvv(i16x8_min_u,             128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 663; testcase_vvv(i16x8_min_u,             128'h007b007b007b007b007b007b007b007b, 128'h007b007b007b007b007b007b007b007b, 128'h007b007b007b007b007b007b007b007b);
   cn = 664; testcase_vvv(i16x8_min_u,             128'h00800080008000800080008000800080, 128'h00800080008000800080008000800080, 128'h00800080008000800080008000800080);
   cn = 665; testcase_vvv(i32x4_min_s,             128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 666; testcase_vvv(i32x4_min_s,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 667; testcase_vvv(i32x4_min_s,             128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h00000000ffffffffffffffffffffffff);
   cn = 668; testcase_vvv(i32x4_min_s,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 669; testcase_vvv(i32x4_min_s,             128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001);
   cn = 670; testcase_vvv(i32x4_min_s,             128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001, 128'hffffffffffffffffffffffffffffffff);
   cn = 671; testcase_vvv(i32x4_min_s,             128'hffffffffffffffffffffffffffffffff, 128'h00000080000000800000008000000080, 128'hffffffffffffffffffffffffffffffff);
   cn = 672; testcase_vvv(i32x4_min_s,             128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000);
   cn = 673; testcase_vvv(i32x4_min_s,             128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000);
   cn = 674; testcase_vvv(i32x4_min_s,             128'h0000007b0000007b0000007b0000007b, 128'h0000007b0000007b0000007b0000007b, 128'h0000007b0000007b0000007b0000007b);
   cn = 675; testcase_vvv(i32x4_min_s,             128'h00000080000000800000008000000080, 128'h00000080000000800000008000000080, 128'h00000080000000800000008000000080);
   cn = 676; testcase_vvv(i32x4_min_u,             128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 677; testcase_vvv(i32x4_min_u,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 678; testcase_vvv(i32x4_min_u,             128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h000000000000000000000000ffffffff);
   cn = 679; testcase_vvv(i32x4_min_u,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 680; testcase_vvv(i32x4_min_u,             128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001);
   cn = 681; testcase_vvv(i32x4_min_u,             128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001);
   cn = 682; testcase_vvv(i32x4_min_u,             128'hffffffffffffffffffffffffffffffff, 128'h00000080000000800000008000000080, 128'h00000080000000800000008000000080);
   cn = 683; testcase_vvv(i32x4_min_u,             128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000);
   cn = 684; testcase_vvv(i32x4_min_u,             128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000);
   cn = 685; testcase_vvv(i32x4_min_u,             128'h0000007b0000007b0000007b0000007b, 128'h0000007b0000007b0000007b0000007b, 128'h0000007b0000007b0000007b0000007b);
   cn = 686; testcase_vvv(i32x4_min_u,             128'h00000080000000800000008000000080, 128'h00000080000000800000008000000080, 128'h00000080000000800000008000000080);

   $display(" Testing  max");
   cn = 687; testcase_vvv(i8x16_max_s,             128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 688; testcase_vvv(i8x16_max_s,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 689; testcase_vvv(i8x16_max_s,             128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h000000000000000000000000ffffffff);
   cn = 690; testcase_vvv(i8x16_max_s,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 691; testcase_vvv(i8x16_max_s,             128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101);
   cn = 692; testcase_vvv(i8x16_max_s,             128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101);
   cn = 693; testcase_vvv(i8x16_max_s,             128'hffffffffffffffffffffffffffffffff, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 694; testcase_vvv(i8x16_max_s,             128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 695; testcase_vvv(i8x16_max_s,             128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 696; testcase_vvv(i8x16_max_s,             128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b, 128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b, 128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b);
   cn = 697; testcase_vvv(i8x16_max_s,             128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 698; testcase_vvv(i8x16_max_u,             128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 699; testcase_vvv(i8x16_max_u,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 700; testcase_vvv(i8x16_max_u,             128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h00000000ffffffffffffffffffffffff);
   cn = 701; testcase_vvv(i8x16_max_u,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 702; testcase_vvv(i8x16_max_u,             128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101);
   cn = 703; testcase_vvv(i8x16_max_u,             128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff);
   cn = 704; testcase_vvv(i8x16_max_u,             128'hffffffffffffffffffffffffffffffff, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 705; testcase_vvv(i8x16_max_u,             128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 706; testcase_vvv(i8x16_max_u,             128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 707; testcase_vvv(i8x16_max_u,             128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b, 128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b, 128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b);
   cn = 708; testcase_vvv(i8x16_max_u,             128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 709; testcase_vvv(i16x8_max_s,             128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 710; testcase_vvv(i16x8_max_s,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 711; testcase_vvv(i16x8_max_s,             128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h000000000000000000000000ffffffff);
   cn = 712; testcase_vvv(i16x8_max_s,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 713; testcase_vvv(i16x8_max_s,             128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001);
   cn = 714; testcase_vvv(i16x8_max_s,             128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001);
   cn = 715; testcase_vvv(i16x8_max_s,             128'hffffffffffffffffffffffffffffffff, 128'h00800080008000800080008000800080, 128'h00800080008000800080008000800080);
   cn = 716; testcase_vvv(i16x8_max_s,             128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 717; testcase_vvv(i16x8_max_s,             128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 718; testcase_vvv(i16x8_max_s,             128'h007b007b007b007b007b007b007b007b, 128'h007b007b007b007b007b007b007b007b, 128'h007b007b007b007b007b007b007b007b);
   cn = 719; testcase_vvv(i16x8_max_s,             128'h00800080008000800080008000800080, 128'h00800080008000800080008000800080, 128'h00800080008000800080008000800080);
   cn = 720; testcase_vvv(i16x8_max_u,             128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 721; testcase_vvv(i16x8_max_u,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 722; testcase_vvv(i16x8_max_u,             128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h00000000ffffffffffffffffffffffff);
   cn = 723; testcase_vvv(i16x8_max_u,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 724; testcase_vvv(i16x8_max_u,             128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001);
   cn = 725; testcase_vvv(i16x8_max_u,             128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff);
   cn = 726; testcase_vvv(i16x8_max_u,             128'hffffffffffffffffffffffffffffffff, 128'h00800080008000800080008000800080, 128'hffffffffffffffffffffffffffffffff);
   cn = 727; testcase_vvv(i16x8_max_u,             128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 728; testcase_vvv(i16x8_max_u,             128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 729; testcase_vvv(i16x8_max_u,             128'h007b007b007b007b007b007b007b007b, 128'h007b007b007b007b007b007b007b007b, 128'h007b007b007b007b007b007b007b007b);
   cn = 730; testcase_vvv(i16x8_max_u,             128'h00800080008000800080008000800080, 128'h00800080008000800080008000800080, 128'h00800080008000800080008000800080);
   cn = 731; testcase_vvv(i32x4_max_s,             128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 732; testcase_vvv(i32x4_max_s,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 733; testcase_vvv(i32x4_max_s,             128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h000000000000000000000000ffffffff);
   cn = 734; testcase_vvv(i32x4_max_s,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 735; testcase_vvv(i32x4_max_s,             128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001);
   cn = 736; testcase_vvv(i32x4_max_s,             128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001);
   cn = 737; testcase_vvv(i32x4_max_s,             128'hffffffffffffffffffffffffffffffff, 128'h00000080000000800000008000000080, 128'h00000080000000800000008000000080);
   cn = 738; testcase_vvv(i32x4_max_s,             128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000);
   cn = 739; testcase_vvv(i32x4_max_s,             128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000);
   cn = 740; testcase_vvv(i32x4_max_s,             128'h0000007b0000007b0000007b0000007b, 128'h0000007b0000007b0000007b0000007b, 128'h0000007b0000007b0000007b0000007b);
   cn = 741; testcase_vvv(i32x4_max_s,             128'h00000080000000800000008000000080, 128'h00000080000000800000008000000080, 128'h00000080000000800000008000000080);
   cn = 742; testcase_vvv(i32x4_max_u,             128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 743; testcase_vvv(i32x4_max_u,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 744; testcase_vvv(i32x4_max_u,             128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h00000000ffffffffffffffffffffffff);
   cn = 745; testcase_vvv(i32x4_max_u,             128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 746; testcase_vvv(i32x4_max_u,             128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001);
   cn = 747; testcase_vvv(i32x4_max_u,             128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001, 128'hffffffffffffffffffffffffffffffff);
   cn = 748; testcase_vvv(i32x4_max_u,             128'hffffffffffffffffffffffffffffffff, 128'h00000080000000800000008000000080, 128'hffffffffffffffffffffffffffffffff);
   cn = 749; testcase_vvv(i32x4_max_u,             128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000);
   cn = 750; testcase_vvv(i32x4_max_u,             128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000);
   cn = 751; testcase_vvv(i32x4_max_u,             128'h0000007b0000007b0000007b0000007b, 128'h0000007b0000007b0000007b0000007b, 128'h0000007b0000007b0000007b0000007b);
   cn = 752; testcase_vvv(i32x4_max_u,             128'h00000080000000800000008000000080, 128'h00000080000000800000008000000080, 128'h00000080000000800000008000000080);
   $display(" Testing  avgr");
   cn = 755; testcase_vvv(i8x16_avgr_u,            128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 756; testcase_vvv(i8x16_avgr_u,            128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h80808080808080808080808080808080);
   cn = 757; testcase_vvv(i8x16_avgr_u,            128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h000000008080808080808080ffffffff);
   cn = 758; testcase_vvv(i8x16_avgr_u,            128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h80808080808080808080808080808080);
   cn = 759; testcase_vvv(i8x16_avgr_u,            128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101);
   cn = 760; testcase_vvv(i8x16_avgr_u,            128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101, 128'h80808080808080808080808080808080);
   cn = 761; testcase_vvv(i8x16_avgr_u,            128'hffffffffffffffffffffffffffffffff, 128'h80808080808080808080808080808080, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0);
   cn = 762; testcase_vvv(i8x16_avgr_u,            128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 763; testcase_vvv(i8x16_avgr_u,            128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 764; testcase_vvv(i8x16_avgr_u,            128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b, 128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b, 128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b);
   cn = 765; testcase_vvv(i8x16_avgr_u,            128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 766; testcase_vvv(i16x8_avgr_u,            128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 767; testcase_vvv(i16x8_avgr_u,            128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h80008000800080008000800080008000);
   cn = 768; testcase_vvv(i16x8_avgr_u,            128'h0000000000000000ffffffffffffffff, 128'h00000000ffffffff00000000ffffffff, 128'h000000008000800080008000ffffffff);
   cn = 769; testcase_vvv(i16x8_avgr_u,            128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h80008000800080008000800080008000);
   cn = 770; testcase_vvv(i16x8_avgr_u,            128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001);
   cn = 771; testcase_vvv(i16x8_avgr_u,            128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'h80008000800080008000800080008000);
   cn = 772; testcase_vvv(i16x8_avgr_u,            128'hffffffffffffffffffffffffffffffff, 128'h00800080008000800080008000800080, 128'h80408040804080408040804080408040);
   cn = 773; testcase_vvv(i16x8_avgr_u,            128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 774; testcase_vvv(i16x8_avgr_u,            128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 775; testcase_vvv(i16x8_avgr_u,            128'h007b007b007b007b007b007b007b007b, 128'h007b007b007b007b007b007b007b007b, 128'h007b007b007b007b007b007b007b007b);
   cn = 776; testcase_vvv(i16x8_avgr_u,            128'h00800080008000800080008000800080, 128'h00800080008000800080008000800080, 128'h00800080008000800080008000800080);


`ifdef   ENABLE_NEGABSSAT
   $display(" Testing  abs");
   cn = 916; testcase_vv(i8x16_abs,                128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101);
   cn = 917; testcase_vv(i8x16_abs,                128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101);
   cn = 920; testcase_vv(i8x16_abs,                128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 921; testcase_vv(i8x16_abs,                128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 924; testcase_vv(i8x16_abs,                128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b, 128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b);
   cn = 925; testcase_vv(i8x16_abs,                128'h85858585858585858585858585858585, 128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b);
   cn = 930; testcase_vv(i16x8_abs,                128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001);
   cn = 931; testcase_vv(i16x8_abs,                128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001);
   cn = 934; testcase_vv(i16x8_abs,                128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 938; testcase_vv(i16x8_abs,                128'h007b007b007b007b007b007b007b007b, 128'h007b007b007b007b007b007b007b007b);
   cn = 939; testcase_vv(i16x8_abs,                128'hff85ff85ff85ff85ff85ff85ff85ff85, 128'h007b007b007b007b007b007b007b007b);
   cn = 940; testcase_vv(i16x8_abs,                128'h00800080008000800080008000800080, 128'h00800080008000800080008000800080);
   cn = 941; testcase_vv(i16x8_abs,                128'hff80ff80ff80ff80ff80ff80ff80ff80, 128'h00800080008000800080008000800080);
   cn = 942; testcase_vv(i16x8_abs,                128'h00800080008000800080008000800080, 128'h00800080008000800080008000800080);
   cn = 943; testcase_vv(i16x8_abs,                128'hff80ff80ff80ff80ff80ff80ff80ff80, 128'h00800080008000800080008000800080);
   cn = 944; testcase_vv(i32x4_abs,                128'h00000001000000010000000100000001, 128'h00000001000000010000000100000001);
   cn = 945; testcase_vv(i32x4_abs,                128'hffffffffffffffffffffffffffffffff, 128'h00000001000000010000000100000001);
   cn = 948; testcase_vv(i32x4_abs,                128'h80000000800000008000000080000000, 128'h80000000800000008000000080000000);
   cn = 952; testcase_vv(i32x4_abs,                128'h0000007b0000007b0000007b0000007b, 128'h0000007b0000007b0000007b0000007b);
   cn = 953; testcase_vv(i32x4_abs,                128'hffffff85ffffff85ffffff85ffffff85, 128'h0000007b0000007b0000007b0000007b);
   cn = 954; testcase_vv(i32x4_abs,                128'h00000080000000800000008000000080, 128'h00000080000000800000008000000080);
   cn = 955; testcase_vv(i32x4_abs,                128'hffffff80ffffff80ffffff80ffffff80, 128'h00000080000000800000008000000080);
   cn = 956; testcase_vv(i32x4_abs,                128'h00000080000000800000008000000080, 128'h00000080000000800000008000000080);
   cn = 957; testcase_vv(i32x4_abs,                128'hffffff80ffffff80ffffff80ffffff80, 128'h00000080000000800000008000000080);
`endif //ENABLE_NEGABSSAT

   $display(" Testing  eq");
   cn = 958; testcase_vvv(i8x16_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 959; testcase_vvv(i8x16_eq,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 960; testcase_vvv(i8x16_eq,                128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 961; testcase_vvv(i8x16_eq,                128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 962; testcase_vvv(i8x16_eq,                128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 963; testcase_vvv(i8x16_eq,                128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 964; testcase_vvv(i8x16_eq,                128'h0001020304091011120a0b1a1baaabff, 128'h0001020304091011120a0b1a1baaabff, 128'hffffffffffffffffffffffffffffffff);
   cn = 965; testcase_vvv(i8x16_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 966; testcase_vvv(i8x16_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 967; testcase_vvv(i8x16_eq,                128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 968; testcase_vvv(i8x16_eq,                128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 969; testcase_vvv(i8x16_eq,                128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'hffffffffffffffffffffffffffffffff);
   cn = 970; testcase_vvv(i8x16_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 971; testcase_vvv(i8x16_eq,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 972; testcase_vvv(i8x16_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 973; testcase_vvv(i8x16_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 974; testcase_vvv(i8x16_eq,                128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 975; testcase_vvv(i8x16_eq,                128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 976; testcase_vvv(i8x16_eq,                128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'hffffffffffffffffffffffffffffffff);
   cn = 977; testcase_vvv(i8x16_eq,                128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 978; testcase_vvv(i8x16_eq,                128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 979; testcase_vvv(i8x16_eq,                128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'h00000000000000000000000000000000);
   cn = 980; testcase_vvv(i8x16_eq,                128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'h00000000000000ffff00000000000000);
   cn = 981; testcase_vvv(i8x16_eq,                128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'h00000000000000ffff00000000000000);
   cn = 982; testcase_vvv(i8x16_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 983; testcase_vvv(i8x16_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 984; testcase_vvv(i8x16_eq,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 985; testcase_vvv(i8x16_eq,                128'h000102030405060708090a0b0c0d0e0f, 128'h000102030405060708090a0b0c0d0e0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 986; testcase_vvv(i8x16_eq,                128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'hffffffffffffffffffffffffffffffff);
   cn = 988; testcase_vvv(i8x16_eq,                128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 989; testcase_vvv(i8x16_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 990; testcase_vvv(i8x16_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 991; testcase_vvv(i8x16_eq,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 992; testcase_vvv(i8x16_eq,                128'h03020100070605040b0a09080f0e0d0c, 128'h03020100070605040b0a09080f0e0d0c, 128'hffffffffffffffffffffffffffffffff);
   cn = 993; testcase_vvv(i8x16_eq,                128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'hffffffffffffffffffffffffffffffff);
   cn = 995; testcase_vvv(i8x16_eq,                128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1609; testcase_vvv(i16x8_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1610; testcase_vvv(i16x8_eq,                128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1611; testcase_vvv(i16x8_eq,                128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1612; testcase_vvv(i16x8_eq,                128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1613; testcase_vvv(i16x8_eq,                128'hffff_ffff_0000_0000_0001_0001_8000_8000, 128'h0000_ffff_0000_0000_0000_0001_0000_8000, 128'h0000_ffff_ffff_ffff_0000_ffff_0000_ffff);
   cn = 1614; testcase_vvv(i16x8_eq,                128'hff80ff80000000000001000100ff00ff, 128'h808080800000000001010101ffffffff, 128'h00000000ffffffff0000000000000000);
   cn = 1615; testcase_vvv(i16x8_eq,                128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1616; testcase_vvv(i16x8_eq,                128'h8180_8382_fefd_00ff_0100_7f02_fd80_fffe, 128'h8382_8180_00ff_fefd_7f02_0100_fffe_fd80, 128'h0000_0000_0000_0000_0000_0000_0000_0000);
   cn = 1617; testcase_vvv(i16x8_eq,                128'h81808382fefd00ff01007f02fd80fffe, 128'h81808382fefd00ff01007f02fd80fffe, 128'hffffffffffffffffffffffffffffffff);
   cn = 1618; testcase_vvv(i16x8_eq,                128'h8180_8382_fefd_00ff_0100_7f02_fd80_fffe, 128'h8081_8283_fdfe_ff00_0001_027f_80fd_feff, 128'h0000_0000_0000_0000_0000_0000_0000_0000);
   cn = 1619; testcase_vvv(i16x8_eq,                128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1620; testcase_vvv(i16x8_eq,                128'h8000fffeffff0000000000010002ffff, 128'h8000fffeffff0000000000010002ffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1621; testcase_vvv(i16x8_eq,                128'h8000fffeffff0000000000010002ffff, 128'h8000fffeffff0000000000010002ffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1622; testcase_vvv(i16x8_eq,                128'h80008001fffeffff0000ffff80018000, 128'h80008001ffff0000fffffffe80018000, 128'hffffffff0000000000000000ffffffff);
   cn = 1623; testcase_vvv(i16x8_eq,                128'h80008001800280038004800580068007, 128'h80078006800580048003800280018000, 128'h00000000000000000000000000000000);
   cn = 1624; testcase_vvv(i16x8_eq,                128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1625; testcase_vvv(i16x8_eq,                128'h30393039303930393039303930393039, 128'h30393039303930393039303930393039, 128'hffffffffffffffffffffffffffffffff);
   cn = 1626; testcase_vvv(i16x8_eq,                128'h12341234123412341234123412341234, 128'h12341234123412341234123412341234, 128'hffffffffffffffffffffffffffffffff);
   cn = 1627; testcase_vvv(i16x8_eq,                128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1628; testcase_vvv(i16x8_eq,                128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1629; testcase_vvv(i16x8_eq,                128'h01000302090411100a121a0baa1bffab, 128'h01000302090411100a121a0baa1bffab, 128'hffffffffffffffffffffffffffffffff);
   cn = 1630; testcase_vvv(i16x8_eq,                128'h010003020504070609080b0a0d0c0f0e, 128'h03020100070605040b0a09080f0e0d0c, 128'h00000000000000000000000000000000);
   cn = 1631; testcase_vvv(i16x8_eq,                128'h010003020504070609080b0a0d0c0f0e, 128'h000102030405060708090a0b0c0d0e0f, 128'h00000000000000000000000000000000);
   cn = 1632; testcase_vvv(i16x8_eq,                128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'h00000000000000000000000000000000);
   cn = 1633; testcase_vvv(i16x8_eq,                128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1634; testcase_vvv(i16x8_eq,                128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1635; testcase_vvv(i16x8_eq,                128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1636; testcase_vvv(i16x8_eq,                128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1637; testcase_vvv(i16x8_eq,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1638; testcase_vvv(i32x4_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1639; testcase_vvv(i32x4_eq,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1640; testcase_vvv(i32x4_eq,                128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1641; testcase_vvv(i32x4_eq,                128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1642; testcase_vvv(i32x4_eq,                128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1643; testcase_vvv(i32x4_eq,                128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1644; testcase_vvv(i32x4_eq,                128'h03020100111009041a0b0a12ffabaa1b, 128'h03020100111009041a0b0a12ffabaa1b, 128'hffffffffffffffffffffffffffffffff);
   cn = 1645; testcase_vvv(i32x4_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1646; testcase_vvv(i32x4_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1647; testcase_vvv(i32x4_eq,                128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1648; testcase_vvv(i32x4_eq,                128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1649; testcase_vvv(i32x4_eq,                128'h8382818000fffefd7f020100fffefd80, 128'h8382818000fffefd7f020100fffefd80, 128'hffffffffffffffffffffffffffffffff);
   cn = 1650; testcase_vvv(i32x4_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1651; testcase_vvv(i32x4_eq,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1652; testcase_vvv(i32x4_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1653; testcase_vvv(i32x4_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1654; testcase_vvv(i32x4_eq,                128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1655; testcase_vvv(i32x4_eq,                128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1656; testcase_vvv(i32x4_eq,                128'h80000001ffffffff00000000ffffffff, 128'h80000001ffffffff00000000ffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1657; testcase_vvv(i32x4_eq,                128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1658; testcase_vvv(i32x4_eq,                128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1659; testcase_vvv(i32x4_eq,                128'h02030001101104090b1a120aabff1baa, 128'haa1bffab0a121a0b0904111001000302, 128'h00000000000000000000000000000000);
   cn = 1660; testcase_vvv(i32x4_eq,                128'h80018000800380028005800480078006, 128'h80078006800580048003800280018000, 128'h00000000000000000000000000000000);
   cn = 1661; testcase_vvv(i32x4_eq,                128'h800000007fffffff00000000ffffffff, 128'h8000000080000001ffffffff00000000, 128'hffffffff000000000000000000000000);
   cn = 1662; testcase_vvv(i32x4_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1663; testcase_vvv(i32x4_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1664; testcase_vvv(i32x4_eq,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1665; testcase_vvv(i32x4_eq,                128'h03020100070605040b0a09080f0e0d0c, 128'h000102030405060708090a0b0c0d0e0f, 128'h00000000000000000000000000000000);
   cn = 1666; testcase_vvv(i32x4_eq,                128'h83828180_00fffefd_7f020100_fffefd80, 128'h80818283_fdfeff00_0001027f_80fdfeff, 128'h00000000_00000000_00000000_00000000);
   cn = 1667; testcase_vvv(i32x4_eq,                128'hff80ff800000000000000001ffffffff, 128'h808080800000000001010101ffffffff, 128'h00000000ffffffff00000000ffffffff);
   cn = 1668; testcase_vvv(i32x4_eq,                128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1669; testcase_vvv(i32x4_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1670; testcase_vvv(i32x4_eq,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1671; testcase_vvv(i32x4_eq,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1672; testcase_vvv(i32x4_eq,                128'h03020100070605040b0a09080f0e0d0c, 128'h010003020504070609080b0a0d0c0f0e, 128'h00000000000000000000000000000000);
   cn = 1673; testcase_vvv(i32x4_eq,                128'h83828180_00fffefd_7f020100_fffefd80, 128'h81808382_fefd00ff_01007f02_fd80fffe, 128'h00000000_00000000_00000000_00000000);
   cn = 1674; testcase_vvv(i32x4_eq,                128'hffffffff_00000000_00000001_0000ffff, 128'hffffffff_00000000_00010000_ffffffff, 128'hffffffff_ffffffff_00000000_00000000);
   cn = 1675; testcase_vvv(i32x4_eq,                128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1676; testcase_vvv(i32x4_eq,                128'h075bcd15075bcd15075bcd15075bcd15, 128'h075bcd15075bcd15075bcd15075bcd15, 128'hffffffffffffffffffffffffffffffff);
   cn = 1677; testcase_vvv(i32x4_eq,                128'h12345678123456781234567812345678, 128'h12345678123456781234567812345678, 128'hffffffffffffffffffffffffffffffff);
   `ifdef  ALL_RELATIONALS
   cn = 996; testcase_vvv(i8x16_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 997; testcase_vvv(i8x16_ne,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 998; testcase_vvv(i8x16_ne,                128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 999; testcase_vvv(i8x16_ne,                128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h00000000000000000000000000000000);
   cn = 1000; testcase_vvv(i8x16_ne,                128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1001; testcase_vvv(i8x16_ne,                128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1002; testcase_vvv(i8x16_ne,                128'h0001020304091011120a0b1a1baaabff, 128'h0001020304091011120a0b1a1baaabff, 128'h00000000000000000000000000000000);
   cn = 1003; testcase_vvv(i8x16_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1004; testcase_vvv(i8x16_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1005; testcase_vvv(i8x16_ne,                128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1006; testcase_vvv(i8x16_ne,                128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1007; testcase_vvv(i8x16_ne,                128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'h00000000000000000000000000000000);
   cn = 1008; testcase_vvv(i8x16_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1009; testcase_vvv(i8x16_ne,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1000; testcase_vvv(i8x16_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1011; testcase_vvv(i8x16_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1012; testcase_vvv(i8x16_ne,                128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1013; testcase_vvv(i8x16_ne,                128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1014; testcase_vvv(i8x16_ne,                128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'h00000000000000000000000000000000);
   cn = 1015; testcase_vvv(i8x16_ne,                128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1016; testcase_vvv(i8x16_ne,                128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1017; testcase_vvv(i8x16_ne,                128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'hffffffffffffffffffffffffffffffff);
   cn = 1018; testcase_vvv(i8x16_ne,                128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'hffffffffffffff0000ffffffffffffff);
   cn = 1019; testcase_vvv(i8x16_ne,                128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'hffffffffffffff0000ffffffffffffff);
   cn = 1010; testcase_vvv(i8x16_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1021; testcase_vvv(i8x16_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1022; testcase_vvv(i8x16_ne,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1026; testcase_vvv(i8x16_ne,                128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn = 1027; testcase_vvv(i8x16_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1028; testcase_vvv(i8x16_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1029; testcase_vvv(i8x16_ne,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1033; testcase_vvv(i8x16_ne,                128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   `endif //ALL_RELATIONALS
   $display(" Testing  lt_s");
   cn = 1034; testcase_vvv(i8x16_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1035; testcase_vvv(i8x16_lt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1036; testcase_vvv(i8x16_lt_s,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1037; testcase_vvv(i8x16_lt_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h00000000000000000000000000000000);
   cn = 1038; testcase_vvv(i8x16_lt_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1039; testcase_vvv(i8x16_lt_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1040; testcase_vvv(i8x16_lt_s,              128'h0001020304091011120a0b1a1baaabff, 128'h0001020304091011120a0b1a1baaabff, 128'h00000000000000000000000000000000);
   cn = 1041; testcase_vvv(i8x16_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1042; testcase_vvv(i8x16_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1043; testcase_vvv(i8x16_lt_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1044; testcase_vvv(i8x16_lt_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1045; testcase_vvv(i8x16_lt_s,              128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'h00000000000000000000000000000000);
   cn = 1046; testcase_vvv(i8x16_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1047; testcase_vvv(i8x16_lt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1048; testcase_vvv(i8x16_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1049; testcase_vvv(i8x16_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1050; testcase_vvv(i8x16_lt_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1051; testcase_vvv(i8x16_lt_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1052; testcase_vvv(i8x16_lt_s,              128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'h00000000000000000000000000000000);
   cn = 1053; testcase_vvv(i8x16_lt_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1054; testcase_vvv(i8x16_lt_s,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'h0000000000000000ffffffffffffffff);
   cn = 1055; testcase_vvv(i8x16_lt_s,              128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'h000000ffffff00ff00ff000000ffffff);
   cn = 1056; testcase_vvv(i8x16_lt_s,              128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'hffffff00ffffff0000000000ff000000);
   cn = 1057; testcase_vvv(i8x16_lt_s,              128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'hffffff00ffffff0000000000ff000000);
   cn = 1058; testcase_vvv(i8x16_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1059; testcase_vvv(i8x16_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1060; testcase_vvv(i8x16_lt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1064; testcase_vvv(i8x16_lt_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1065; testcase_vvv(i8x16_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1066; testcase_vvv(i8x16_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1067; testcase_vvv(i8x16_lt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1061; testcase_vvv(i8x16_lt_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);

   cn = 1398; testcase_vvv(i16x8_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1399; testcase_vvv(i16x8_lt_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1401; testcase_vvv(i16x8_lt_s,              128'hff80ff80000000000001000100ff00ff, 128'h808080800000000001010101ffffffff, 128'h0000000000000000ffffffff00000000);
   cn = 1402; testcase_vvv(i16x8_lt_s,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1404; testcase_vvv(i16x8_lt_s,              128'h81808382fefd00ff01007f02fd80fffe, 128'h81808382fefd00ff01007f02fd80fffe, 128'h00000000000000000000000000000000);
   cn = 1405; testcase_vvv(i16x8_lt_s,              128'h81808382fefd00ff01007f02fd80fffe, 128'h80818283fdfeff000001027f80fdfeff, 128'h00000000000000000000000000000000);
   cn = 1406; testcase_vvv(i16x8_lt_s,              128'h80818283fdfeff000001027f80fdfeff, 128'hfeff80fd027f0001ff00fdfe82838081, 128'hffff0000ffffffff00000000ffff0000);
   cn = 1407; testcase_vvv(i16x8_lt_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1408; testcase_vvv(i16x8_lt_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1409; testcase_vvv(i16x8_lt_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1410; testcase_vvv(i16x8_lt_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1411; testcase_vvv(i16x8_lt_s,              128'h30393039303930393039303930393039, 128'h30393039303930393039303930393039, 128'h00000000000000000000000000000000);
   cn = 1412; testcase_vvv(i16x8_lt_s,              128'h12341234123412341234123412341234, 128'h12341234123412341234123412341234, 128'h00000000000000000000000000000000);
   cn = 1413; testcase_vvv(i16x8_lt_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1414; testcase_vvv(i16x8_lt_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h00000000000000000000000000000000);
   cn = 1415; testcase_vvv(i16x8_lt_s,              128'h01000302090411100a121a0baa1bffab, 128'h01000302090411100a121a0baa1bffab, 128'h00000000000000000000000000000000);
   cn = 1417; testcase_vvv(i16x8_lt_s,              128'h010003020504070609080b0a0d0c0f0e, 128'h000102030405060708090a0b0c0d0e0f, 128'h00000000000000000000000000000000);
   cn = 1418; testcase_vvv(i16x8_lt_s,              128'h00ff7fff0000000000010002fffeffff, 128'h00ff7fff0000000000010002fffeffff, 128'h00000000000000000000000000000000);
   cn = 1419; testcase_vvv(i16x8_lt_s,              128'h00ff00ff00ff00ff00ff00ff00ff00ff, 128'h00ff00ff00ff00ff00ff00ff00ff00ff, 128'h00000000000000000000000000000000);
   cn = 1420; testcase_vvv(i16x8_lt_s,              128'h00ff00ff00ff00ff0000000000000000, 128'h00ff00ff00ff00ff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1421; testcase_vvv(i16x8_lt_s,              128'h0080008100820083000000ff7ffe7fff, 128'h7fff7ffe00ff0000008300820081001c, 128'hffffffffffff0000ffff000000000000);
   cn = 1422; testcase_vvv(i16x8_lt_s,              128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'h00000000ffff0000000000000000ffff);
   cn = 1423; testcase_vvv(i16x8_lt_s,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'h0000000000000000ffffffffffffffff);
   cn = 1424; testcase_vvv(i16x8_lt_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1425; testcase_vvv(i16x8_lt_s,              128'h000000000000000000ff00ff00ff00ff, 128'h000000000000000000ff00ff00ff00ff, 128'h00000000000000000000000000000000);
   cn = 1426; testcase_vvv(i16x8_lt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1427; testcase_vvv(i16x8_lt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1428; testcase_vvv(i16x8_lt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1429; testcase_vvv(i16x8_lt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);

   cn = 1728; testcase_vvv(i32x4_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1729; testcase_vvv(i32x4_lt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1730; testcase_vvv(i32x4_lt_s,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1731; testcase_vvv(i32x4_lt_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h00000000000000000000000000000000);
   cn = 1732; testcase_vvv(i32x4_lt_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1733; testcase_vvv(i32x4_lt_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1734; testcase_vvv(i32x4_lt_s,              128'h03020100111009041a0b0a12ffabaa1b, 128'h03020100111009041a0b0a12ffabaa1b, 128'h00000000000000000000000000000000);
   cn = 1735; testcase_vvv(i32x4_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1736; testcase_vvv(i32x4_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1737; testcase_vvv(i32x4_lt_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1738; testcase_vvv(i32x4_lt_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1739; testcase_vvv(i32x4_lt_s,              128'h8382818000fffefd7f020100fffefd80, 128'h8382818000fffefd7f020100fffefd80, 128'h00000000000000000000000000000000);
   cn = 1740; testcase_vvv(i32x4_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1741; testcase_vvv(i32x4_lt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1742; testcase_vvv(i32x4_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1743; testcase_vvv(i32x4_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1744; testcase_vvv(i32x4_lt_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1745; testcase_vvv(i32x4_lt_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1746; testcase_vvv(i32x4_lt_s,              128'h80000001ffffffff00000000ffffffff, 128'h80000001ffffffff00000000ffffffff, 128'h00000000000000000000000000000000);
   cn = 1747; testcase_vvv(i32x4_lt_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1748; testcase_vvv(i32x4_lt_s,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'h0000000000000000ffffffffffffffff);
   cn = 1749; testcase_vvv(i32x4_lt_s,              128'h02030001101104090b1a120aabff1baa, 128'haa1bffab0a121a0b0904111001000302, 128'h000000000000000000000000ffffffff);
   cn = 1750; testcase_vvv(i32x4_lt_s,              128'h80018000800380028005800480078006, 128'h80078006800580048003800280018000, 128'hffffffffffffffff0000000000000000);
   cn = 1751; testcase_vvv(i32x4_lt_s,              128'h800000007fffffff00000000ffffffff, 128'h8000000080000001ffffffff00000000, 128'h000000000000000000000000ffffffff);
   cn = 1752; testcase_vvv(i32x4_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1753; testcase_vvv(i32x4_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1754; testcase_vvv(i32x4_lt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1755; testcase_vvv(i32x4_lt_s,              128'h03020100070605040b0a09080f0e0d0c, 128'h000102030405060708090a0b0c0d0e0f, 128'h00000000000000000000000000000000);
   cn = 1756; testcase_vvv(i32x4_lt_s,              128'h8382818000fffefd7f020100fffefd80, 128'h80818283fdfeff000001027f80fdfeff, 128'h00000000000000000000000000000000);
   cn = 1757; testcase_vvv(i32x4_lt_s,              128'hff80ff800000000000000001ffffffff, 128'h808080800000000001010101ffffffff, 128'h0000000000000000ffffffff00000000);
   cn = 1758; testcase_vvv(i32x4_lt_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1759; testcase_vvv(i32x4_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1760; testcase_vvv(i32x4_lt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1761; testcase_vvv(i32x4_lt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1762; testcase_vvv(i32x4_lt_s,              128'h03020100070605040b0a09080f0e0d0c, 128'h010003020504070609080b0a0d0c0f0e, 128'h00000000000000000000000000000000);
   cn = 1763; testcase_vvv(i32x4_lt_s,              128'h8382818000fffefd7f020100fffefd80, 128'h81808382fefd00ff01007f02fd80fffe, 128'h00000000000000000000000000000000);
   cn = 1764; testcase_vvv(i32x4_lt_s,              128'hffffff800000000000000001000000ff, 128'hff80ff80000000000001000100ff00ff, 128'h0000000000000000ffffffffffffffff);
   cn = 1765; testcase_vvv(i32x4_lt_s,              128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h55555555555555555555555555555555, 128'hffffffffffffffffffffffffffffffff);
   cn = 1766; testcase_vvv(i32x4_lt_s,              128'h075bcd15075bcd15075bcd15075bcd15, 128'h075bcd15075bcd15075bcd15075bcd15, 128'h00000000000000000000000000000000);
   cn = 1767; testcase_vvv(i32x4_lt_s,              128'h90abcdef90abcdef90abcdef90abcdef, 128'h90abcdf090abcdf090abcdf090abcdf0, 128'hffffffffffffffffffffffffffffffff);

   $display(" Testing  lt_u");
   cn = 1062; testcase_vvv(i8x16_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1063; testcase_vvv(i8x16_lt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1064; testcase_vvv(i8x16_lt_u,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1065; testcase_vvv(i8x16_lt_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h00000000000000000000000000000000);
   cn = 1066; testcase_vvv(i8x16_lt_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1067; testcase_vvv(i8x16_lt_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1068; testcase_vvv(i8x16_lt_u,              128'h0001020304091011120a0b1a1baaabff, 128'h0001020304091011120a0b1a1baaabff, 128'h00000000000000000000000000000000);
   cn = 1069; testcase_vvv(i8x16_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1070; testcase_vvv(i8x16_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1071; testcase_vvv(i8x16_lt_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1072; testcase_vvv(i8x16_lt_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1073; testcase_vvv(i8x16_lt_u,              128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'h00000000000000000000000000000000);
   cn = 1074; testcase_vvv(i8x16_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1075; testcase_vvv(i8x16_lt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1076; testcase_vvv(i8x16_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1077; testcase_vvv(i8x16_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1078; testcase_vvv(i8x16_lt_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1079; testcase_vvv(i8x16_lt_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1080; testcase_vvv(i8x16_lt_u,              128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'h00000000000000000000000000000000);
   cn = 1081; testcase_vvv(i8x16_lt_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1082; testcase_vvv(i8x16_lt_u,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000);
   cn = 1083; testcase_vvv(i8x16_lt_u,              128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'hffffffffffff00ff00ff000000000000);
   cn = 1084; testcase_vvv(i8x16_lt_u,              128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'hffffff000000000000ffffffff000000);
   cn = 1085; testcase_vvv(i8x16_lt_u,              128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'hffffff000000000000ffffffff000000);
   cn = 1086; testcase_vvv(i8x16_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1087; testcase_vvv(i8x16_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1088; testcase_vvv(i8x16_lt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1092; testcase_vvv(i8x16_lt_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn = 1093; testcase_vvv(i8x16_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1094; testcase_vvv(i8x16_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1095; testcase_vvv(i8x16_lt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1099; testcase_vvv(i8x16_lt_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);

   cn = 1358; testcase_vvv(i16x8_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1367; testcase_vvv(i16x8_lt_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1369; testcase_vvv(i16x8_lt_u,              128'hff80ff80000000000001000100ff00ff, 128'h808080800000000001010101ffffffff, 128'h0000000000000000ffffffffffffffff);
   cn = 1370; testcase_vvv(i16x8_lt_u,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1372; testcase_vvv(i16x8_lt_u,              128'h81808382fefd00ff01007f02fd80fffe, 128'h81808382fefd00ff01007f02fd80fffe, 128'h00000000000000000000000000000000);
   cn = 1374; testcase_vvv(i16x8_lt_u,              128'h80818283fdfeff000001027f80fdfeff, 128'hfeff80fd027f0001ff00fdfe82838081, 128'hffff000000000000ffffffffffff0000);
   cn = 1375; testcase_vvv(i16x8_lt_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1376; testcase_vvv(i16x8_lt_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1377; testcase_vvv(i16x8_lt_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn = 1378; testcase_vvv(i16x8_lt_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn = 1379; testcase_vvv(i16x8_lt_u,              128'h30393039303930393039303930393039, 128'h30393039303930393039303930393039, 128'h00000000000000000000000000000000);
   cn = 1380; testcase_vvv(i16x8_lt_u,              128'h12341234123412341234123412341234, 128'h12341234123412341234123412341234, 128'h00000000000000000000000000000000);
   cn = 1381; testcase_vvv(i16x8_lt_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1382; testcase_vvv(i16x8_lt_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h00000000000000000000000000000000);
   cn = 1383; testcase_vvv(i16x8_lt_u,              128'h01000302090411100a121a0baa1bffab, 128'h01000302090411100a121a0baa1bffab, 128'h00000000000000000000000000000000);
   cn = 1385; testcase_vvv(i16x8_lt_u,              128'h010003020504070609080b0a0d0c0f0e, 128'h000102030405060708090a0b0c0d0e0f, 128'h00000000000000000000000000000000);
   cn = 1386; testcase_vvv(i16x8_lt_u,              128'h00ff7fff0000000000010002fffeffff, 128'h00ff7fff0000000000010002fffeffff, 128'h00000000000000000000000000000000);
   cn = 1387; testcase_vvv(i16x8_lt_u,              128'h00ff00ff00ff00ff00ff00ff00ff00ff, 128'h00ff00ff00ff00ff00ff00ff00ff00ff, 128'h00000000000000000000000000000000);
   cn = 1388; testcase_vvv(i16x8_lt_u,              128'h00ff00ff00ff00ff0000000000000000, 128'h00ff00ff00ff00ff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1389; testcase_vvv(i16x8_lt_u,              128'h0080008100820083000000ff7ffe7fff, 128'h7fff7ffe00ff0000008300820081001c, 128'hffffffffffff0000ffff000000000000);
   cn = 1390; testcase_vvv(i16x8_lt_u,              128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'hffffffffffff00000000000000000000);
   cn = 1391; testcase_vvv(i16x8_lt_u,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000);
   cn = 1392; testcase_vvv(i16x8_lt_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1393; testcase_vvv(i16x8_lt_u,              128'h000000000000000000ff00ff00ff00ff, 128'h000000000000000000ff00ff00ff00ff, 128'h00000000000000000000000000000000);
   cn = 1394; testcase_vvv(i16x8_lt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);

   cn = 1768; testcase_vvv(i32x4_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1769; testcase_vvv(i32x4_lt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1770; testcase_vvv(i32x4_lt_u,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1771; testcase_vvv(i32x4_lt_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h00000000000000000000000000000000);
   cn = 1772; testcase_vvv(i32x4_lt_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1773; testcase_vvv(i32x4_lt_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1774; testcase_vvv(i32x4_lt_u,              128'h03020100111009041a0b0a12ffabaa1b, 128'h03020100111009041a0b0a12ffabaa1b, 128'h00000000000000000000000000000000);
   cn = 1775; testcase_vvv(i32x4_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1776; testcase_vvv(i32x4_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1777; testcase_vvv(i32x4_lt_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1778; testcase_vvv(i32x4_lt_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1779; testcase_vvv(i32x4_lt_u,              128'h8382818000fffefd7f020100fffefd80, 128'h8382818000fffefd7f020100fffefd80, 128'h00000000000000000000000000000000);
   cn = 1780; testcase_vvv(i32x4_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1781; testcase_vvv(i32x4_lt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1782; testcase_vvv(i32x4_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1783; testcase_vvv(i32x4_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1784; testcase_vvv(i32x4_lt_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1785; testcase_vvv(i32x4_lt_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1786; testcase_vvv(i32x4_lt_u,              128'h80000001ffffffff00000000ffffffff, 128'h80000001ffffffff00000000ffffffff, 128'h00000000000000000000000000000000);
   cn = 1787; testcase_vvv(i32x4_lt_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1788; testcase_vvv(i32x4_lt_u,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000);
   cn = 1789; testcase_vvv(i32x4_lt_u,              128'h02030001101104090b1a120aabff1baa, 128'haa1bffab0a121a0b0904111001000302, 128'hffffffff000000000000000000000000);
   cn = 1790; testcase_vvv(i32x4_lt_u,              128'h80018000800380028005800480078006, 128'h80078006800580048003800280018000, 128'hffffffffffffffff0000000000000000);
   cn = 1791; testcase_vvv(i32x4_lt_u,              128'h800000007fffffff00000000ffffffff, 128'h8000000080000001ffffffff00000000, 128'h00000000ffffffffffffffff00000000);
   cn = 1792; testcase_vvv(i32x4_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1793; testcase_vvv(i32x4_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1794; testcase_vvv(i32x4_lt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1795; testcase_vvv(i32x4_lt_u,              128'h03020100070605040b0a09080f0e0d0c, 128'h000102030405060708090a0b0c0d0e0f, 128'h00000000000000000000000000000000);
   cn = 1796; testcase_vvv(i32x4_lt_u,              128'h83828180_00fffefd_7f020100_fffefd80, 128'h80818283_fdfeff00_0001027f_80fdfeff, 128'h00000000_ffffffff_00000000_00000000);
   cn = 1797; testcase_vvv(i32x4_lt_u,              128'hff80ff800000000000000001ffffffff, 128'h808080800000000001010101ffffffff, 128'h0000000000000000ffffffff00000000);
   cn = 1798; testcase_vvv(i32x4_lt_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn = 1799; testcase_vvv(i32x4_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1800; testcase_vvv(i32x4_lt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1801; testcase_vvv(i32x4_lt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1802; testcase_vvv(i32x4_lt_u,              128'h03020100070605040b0a09080f0e0d0c, 128'h010003020504070609080b0a0d0c0f0e, 128'h00000000000000000000000000000000);
   cn = 1803; testcase_vvv(i32x4_lt_u,              128'h83828180_00fffefd_7f020100_fffefd80, 128'h81808382_fefd00ff_01007f02_fd80fffe, 128'h00000000_ffffffff_00000000_00000000);
   cn = 1804; testcase_vvv(i32x4_lt_u,              128'hffffff800000000000000001000000ff, 128'hff80ff80000000000001000100ff00ff, 128'h0000000000000000ffffffffffffffff);
   cn = 1805; testcase_vvv(i32x4_lt_u,              128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h55555555555555555555555555555555, 128'h00000000000000000000000000000000);
   cn = 1806; testcase_vvv(i32x4_lt_u,              128'h075bcd15075bcd15075bcd15075bcd15, 128'h075bcd15075bcd15075bcd15075bcd15, 128'h00000000000000000000000000000000);
   cn = 1807; testcase_vvv(i32x4_lt_u,              128'h90abcdef90abcdef90abcdef90abcdef, 128'h90abcdf090abcdf090abcdf090abcdf0, 128'hffffffffffffffffffffffffffffffff);
   `ifdef  ALL_RELATIONALS
   cn = 1100; testcase_vvv(i8x16_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1101; testcase_vvv(i8x16_le_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1102; testcase_vvv(i8x16_le_s,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1103; testcase_vvv(i8x16_le_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1104; testcase_vvv(i8x16_le_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1105; testcase_vvv(i8x16_le_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1106; testcase_vvv(i8x16_le_s,              128'h0001020304091011120a0b1a1baaabff, 128'h0001020304091011120a0b1a1baaabff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1107; testcase_vvv(i8x16_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1108; testcase_vvv(i8x16_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1109; testcase_vvv(i8x16_le_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1110; testcase_vvv(i8x16_le_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1111; testcase_vvv(i8x16_le_s,              128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1112; testcase_vvv(i8x16_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1113; testcase_vvv(i8x16_le_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1114; testcase_vvv(i8x16_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1115; testcase_vvv(i8x16_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1116; testcase_vvv(i8x16_le_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1117; testcase_vvv(i8x16_le_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1118; testcase_vvv(i8x16_le_s,              128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1119; testcase_vvv(i8x16_le_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1120; testcase_vvv(i8x16_le_s,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'h0000000000000000ffffffffffffffff);
   cn = 1121; testcase_vvv(i8x16_le_s,              128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'h000000ffffff00ff00ff000000ffffff);
   cn = 1122; testcase_vvv(i8x16_le_s,              128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'hffffff00ffffffffff000000ff000000);
   cn = 1123; testcase_vvv(i8x16_le_s,              128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'hffffff00ffffffffff000000ff000000);
   cn = 1124; testcase_vvv(i8x16_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1125; testcase_vvv(i8x16_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1126; testcase_vvv(i8x16_le_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1130; testcase_vvv(i8x16_le_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1131; testcase_vvv(i8x16_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1132; testcase_vvv(i8x16_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1133; testcase_vvv(i8x16_le_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1137; testcase_vvv(i8x16_le_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);

   cn = 1138; testcase_vvv(i8x16_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1139; testcase_vvv(i8x16_le_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1140; testcase_vvv(i8x16_le_u,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1141; testcase_vvv(i8x16_le_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1142; testcase_vvv(i8x16_le_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1143; testcase_vvv(i8x16_le_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1144; testcase_vvv(i8x16_le_u,              128'h0001020304091011120a0b1a1baaabff, 128'h0001020304091011120a0b1a1baaabff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1145; testcase_vvv(i8x16_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1146; testcase_vvv(i8x16_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1147; testcase_vvv(i8x16_le_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1148; testcase_vvv(i8x16_le_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1149; testcase_vvv(i8x16_le_u,              128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1150; testcase_vvv(i8x16_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1151; testcase_vvv(i8x16_le_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1152; testcase_vvv(i8x16_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1153; testcase_vvv(i8x16_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1154; testcase_vvv(i8x16_le_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1155; testcase_vvv(i8x16_le_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1156; testcase_vvv(i8x16_le_u,              128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1157; testcase_vvv(i8x16_le_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1158; testcase_vvv(i8x16_le_u,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000);
   cn = 1159; testcase_vvv(i8x16_le_u,              128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'hffffffffffff00ff00ff000000000000);
   cn = 1160; testcase_vvv(i8x16_le_u,              128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'hffffff00000000ffffffffffff000000);
   cn = 1161; testcase_vvv(i8x16_le_u,              128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'hffffff00000000ffffffffffff000000);
   cn = 1162; testcase_vvv(i8x16_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1163; testcase_vvv(i8x16_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1164; testcase_vvv(i8x16_le_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1168; testcase_vvv(i8x16_le_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn = 1169; testcase_vvv(i8x16_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1170; testcase_vvv(i8x16_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1171; testcase_vvv(i8x16_le_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1175; testcase_vvv(i8x16_le_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);

   cn = 1176; testcase_vvv(i8x16_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1177; testcase_vvv(i8x16_gt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1178; testcase_vvv(i8x16_gt_s,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1179; testcase_vvv(i8x16_gt_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h00000000000000000000000000000000);
   cn = 1180; testcase_vvv(i8x16_gt_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1181; testcase_vvv(i8x16_gt_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1182; testcase_vvv(i8x16_gt_s,              128'h0001020304091011120a0b1a1baaabff, 128'h0001020304091011120a0b1a1baaabff, 128'h00000000000000000000000000000000);
   cn = 1183; testcase_vvv(i8x16_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1184; testcase_vvv(i8x16_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1185; testcase_vvv(i8x16_gt_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1186; testcase_vvv(i8x16_gt_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1187; testcase_vvv(i8x16_gt_s,              128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'h00000000000000000000000000000000);
   cn = 1188; testcase_vvv(i8x16_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1189; testcase_vvv(i8x16_gt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1190; testcase_vvv(i8x16_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1191; testcase_vvv(i8x16_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1192; testcase_vvv(i8x16_gt_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1193; testcase_vvv(i8x16_gt_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1194; testcase_vvv(i8x16_gt_s,              128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'h00000000000000000000000000000000);
   cn = 1195; testcase_vvv(i8x16_gt_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1196; testcase_vvv(i8x16_gt_s,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000);
   cn = 1197; testcase_vvv(i8x16_gt_s,              128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'hffffff000000ff00ff00ffffff000000);
   cn = 1198; testcase_vvv(i8x16_gt_s,              128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'h000000ff0000000000ffffff00ffffff);
   cn = 1199; testcase_vvv(i8x16_gt_s,              128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'h000000ff0000000000ffffff00ffffff);
   cn = 1200; testcase_vvv(i8x16_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1201; testcase_vvv(i8x16_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1202; testcase_vvv(i8x16_gt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1206; testcase_vvv(i8x16_gt_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn = 1207; testcase_vvv(i8x16_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1208; testcase_vvv(i8x16_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1209; testcase_vvv(i8x16_gt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1213; testcase_vvv(i8x16_gt_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);

   cn = 1214; testcase_vvv(i8x16_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1215; testcase_vvv(i8x16_gt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1216; testcase_vvv(i8x16_gt_u,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1217; testcase_vvv(i8x16_gt_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h00000000000000000000000000000000);
   cn = 1218; testcase_vvv(i8x16_gt_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1219; testcase_vvv(i8x16_gt_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1220; testcase_vvv(i8x16_gt_u,              128'h0001020304091011120a0b1a1baaabff, 128'h0001020304091011120a0b1a1baaabff, 128'h00000000000000000000000000000000);
   cn = 1221; testcase_vvv(i8x16_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1222; testcase_vvv(i8x16_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1223; testcase_vvv(i8x16_gt_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1224; testcase_vvv(i8x16_gt_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1225; testcase_vvv(i8x16_gt_u,              128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'h00000000000000000000000000000000);
   cn = 1226; testcase_vvv(i8x16_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1227; testcase_vvv(i8x16_gt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1228; testcase_vvv(i8x16_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1229; testcase_vvv(i8x16_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1230; testcase_vvv(i8x16_gt_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1231; testcase_vvv(i8x16_gt_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1232; testcase_vvv(i8x16_gt_u,              128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'h00000000000000000000000000000000);
   cn = 1233; testcase_vvv(i8x16_gt_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1234; testcase_vvv(i8x16_gt_u,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'h0000000000000000ffffffffffffffff);
   cn = 1235; testcase_vvv(i8x16_gt_u,              128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'h000000000000ff00ff00ffffffffffff);
   cn = 1236; testcase_vvv(i8x16_gt_u,              128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'h000000ffffffff000000000000ffffff);
   cn = 1237; testcase_vvv(i8x16_gt_u,              128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'h000000ffffffff000000000000ffffff);
   cn = 1238; testcase_vvv(i8x16_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1239; testcase_vvv(i8x16_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1240; testcase_vvv(i8x16_gt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1244; testcase_vvv(i8x16_gt_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1245; testcase_vvv(i8x16_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1246; testcase_vvv(i8x16_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1247; testcase_vvv(i8x16_gt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1251; testcase_vvv(i8x16_gt_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);

   cn = 1252; testcase_vvv(i8x16_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1253; testcase_vvv(i8x16_ge_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1254; testcase_vvv(i8x16_ge_s,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1255; testcase_vvv(i8x16_ge_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1256; testcase_vvv(i8x16_ge_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1257; testcase_vvv(i8x16_ge_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1258; testcase_vvv(i8x16_ge_s,              128'h0001020304091011120a0b1a1baaabff, 128'h0001020304091011120a0b1a1baaabff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1259; testcase_vvv(i8x16_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1260; testcase_vvv(i8x16_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1261; testcase_vvv(i8x16_ge_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1262; testcase_vvv(i8x16_ge_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1263; testcase_vvv(i8x16_ge_s,              128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1264; testcase_vvv(i8x16_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1265; testcase_vvv(i8x16_ge_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1266; testcase_vvv(i8x16_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1267; testcase_vvv(i8x16_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1268; testcase_vvv(i8x16_ge_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1269; testcase_vvv(i8x16_ge_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1260; testcase_vvv(i8x16_ge_s,              128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1271; testcase_vvv(i8x16_ge_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1272; testcase_vvv(i8x16_ge_s,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000);
   cn = 1273; testcase_vvv(i8x16_ge_s,              128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'hffffff000000ff00ff00ffffff000000);
   cn = 1274; testcase_vvv(i8x16_ge_s,              128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'h000000ff000000ffffffffff00ffffff);
   cn = 1275; testcase_vvv(i8x16_ge_s,              128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'h000000ff000000ffffffffff00ffffff);
   cn = 1276; testcase_vvv(i8x16_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1277; testcase_vvv(i8x16_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1278; testcase_vvv(i8x16_ge_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1282; testcase_vvv(i8x16_ge_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn = 1283; testcase_vvv(i8x16_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1284; testcase_vvv(i8x16_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1285; testcase_vvv(i8x16_ge_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1289; testcase_vvv(i8x16_ge_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);

   cn = 1290; testcase_vvv(i8x16_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1291; testcase_vvv(i8x16_ge_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1292; testcase_vvv(i8x16_ge_u,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1293; testcase_vvv(i8x16_ge_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1294; testcase_vvv(i8x16_ge_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1295; testcase_vvv(i8x16_ge_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1296; testcase_vvv(i8x16_ge_u,              128'h0001020304091011120a0b1a1baaabff, 128'h0001020304091011120a0b1a1baaabff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1297; testcase_vvv(i8x16_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1298; testcase_vvv(i8x16_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1299; testcase_vvv(i8x16_ge_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1300; testcase_vvv(i8x16_ge_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1301; testcase_vvv(i8x16_ge_u,              128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1302; testcase_vvv(i8x16_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1303; testcase_vvv(i8x16_ge_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1304; testcase_vvv(i8x16_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1305; testcase_vvv(i8x16_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1306; testcase_vvv(i8x16_ge_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1307; testcase_vvv(i8x16_ge_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1308; testcase_vvv(i8x16_ge_u,              128'h80818283fdfeff000001027f80fdfeff, 128'h80818283fdfeff000001027f80fdfeff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1309; testcase_vvv(i8x16_ge_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1310; testcase_vvv(i8x16_ge_u,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'h0000000000000000ffffffffffffffff);
   cn = 1311; testcase_vvv(i8x16_ge_u,              128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'h000000000000ff00ff00ffffffffffff);
   cn = 1312; testcase_vvv(i8x16_ge_u,              128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'h000000ffffffffffff00000000ffffff);
   cn = 1313; testcase_vvv(i8x16_ge_u,              128'h80818283fdfeff000001027f80fdfeff, 128'hfffefd807f02010000fffefd83828180, 128'h000000ffffffffffff00000000ffffff);
   cn = 1314; testcase_vvv(i8x16_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1315; testcase_vvv(i8x16_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1316; testcase_vvv(i8x16_ge_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1320; testcase_vvv(i8x16_ge_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1321; testcase_vvv(i8x16_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1322; testcase_vvv(i8x16_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1323; testcase_vvv(i8x16_ge_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1327; testcase_vvv(i8x16_ge_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);

   cn = 1328; testcase_vvv(i16x8_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1329; testcase_vvv(i16x8_ne,                128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1331; testcase_vvv(i16x8_ne,                128'hff80ff80000000000001000100ff00ff, 128'h808080800000000001010101ffffffff, 128'hffffffff00000000ffffffffffffffff);
   cn = 1332; testcase_vvv(i16x8_ne,                128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1334; testcase_vvv(i16x8_ne,                128'h81808382fefd00ff01007f02fd80fffe, 128'h81808382fefd00ff01007f02fd80fffe, 128'h00000000000000000000000000000000);
   cn = 1336; testcase_vvv(i16x8_ne,                128'h80818283fdfeff000001027f80fdfeff, 128'hfeff80fd027f0001ff00fdfe82838081, 128'hffffffffffffffffffffffffffffffff);
   cn = 1337; testcase_vvv(i16x8_ne,                128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1338; testcase_vvv(i16x8_ne,                128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn = 1339; testcase_vvv(i16x8_ne,                128'h30393039303930393039303930393039, 128'h30393039303930393039303930393039, 128'h00000000000000000000000000000000);
   cn = 1340; testcase_vvv(i16x8_ne,                128'h12341234123412341234123412341234, 128'h12341234123412341234123412341234, 128'h00000000000000000000000000000000);
   cn = 1341; testcase_vvv(i16x8_ne,                128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1342; testcase_vvv(i16x8_ne,                128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h00000000000000000000000000000000);
   cn = 1343; testcase_vvv(i16x8_ne,                128'h01000302090411100a121a0baa1bffab, 128'h01000302090411100a121a0baa1bffab, 128'h00000000000000000000000000000000);
   cn = 1346; testcase_vvv(i16x8_ne,                128'h00ff7fff0000000000010002fffeffff, 128'h00ff7fff0000000000010002fffeffff, 128'h00000000000000000000000000000000);
   cn = 1347; testcase_vvv(i16x8_ne,                128'h00ff00ff00ff00ff00ff00ff00ff00ff, 128'h00ff00ff00ff00ff00ff00ff00ff00ff, 128'h00000000000000000000000000000000);
   cn = 1348; testcase_vvv(i16x8_ne,                128'h00ff00ff00ff00ff0000000000000000, 128'h00ff00ff00ff00ff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1349; testcase_vvv(i16x8_ne,                128'h0080008100820083000000ff7ffe7fff, 128'h7fff7ffe00ff0000008300820081001c, 128'hffffffffffffffffffffffffffffffff);
   cn = 1350; testcase_vvv(i16x8_ne,                128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'hffffffffffffffffffffffffffffffff);
   cn = 1351; testcase_vvv(i16x8_ne,                128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1352; testcase_vvv(i16x8_ne,                128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1353; testcase_vvv(i16x8_ne,                128'h000000000000000000ff00ff00ff00ff, 128'h000000000000000000ff00ff00ff00ff, 128'h00000000000000000000000000000000);
   cn = 1354; testcase_vvv(i16x8_ne,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1355; testcase_vvv(i16x8_ne,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1356; testcase_vvv(i16x8_ne,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1357; testcase_vvv(i16x8_ne,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);

   cn = 1430; testcase_vvv(i16x8_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1431; testcase_vvv(i16x8_le_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1432; testcase_vvv(i16x8_le_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1434; testcase_vvv(i16x8_le_u,              128'hff80ff80000000000001000100ff00ff, 128'h808080800000000001010101ffffffff, 128'h00000000ffffffffffffffffffffffff);
   cn = 1435; testcase_vvv(i16x8_le_u,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1436; testcase_vvv(i16x8_le_u,              128'hedcbedcbedcbedcbedcbedcbedcbedcb, 128'hedccedccedccedccedccedccedccedcc, 128'hffffffffffffffffffffffffffffffff);
   cn = 1438; testcase_vvv(i16x8_le_u,              128'h81808382fefd00ff01007f02fd80fffe, 128'h81808382fefd00ff01007f02fd80fffe, 128'hffffffffffffffffffffffffffffffff);
   cn = 1440; testcase_vvv(i16x8_le_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1441; testcase_vvv(i16x8_le_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1442; testcase_vvv(i16x8_le_u,              128'h8000fffeffff0000000000010002ffff, 128'h8000fffeffff0000000000010002ffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1443; testcase_vvv(i16x8_le_u,              128'h80008001fffeffff0000ffff80018000, 128'h80008001ffff0000fffffffe80018000, 128'hffffffffffff0000ffff0000ffffffff);
   cn = 1444; testcase_vvv(i16x8_le_u,              128'h80008001800280038004800580068007, 128'h80078006800580048003800280018000, 128'hffffffffffffffff0000000000000000);
   cn = 1445; testcase_vvv(i16x8_le_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn = 1446; testcase_vvv(i16x8_le_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn = 1447; testcase_vvv(i16x8_le_u,              128'h30393039303930393039303930393039, 128'h30393039303930393039303930393039, 128'hffffffffffffffffffffffffffffffff);
   cn = 1448; testcase_vvv(i16x8_le_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1449; testcase_vvv(i16x8_le_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1450; testcase_vvv(i16x8_le_u,              128'h01000302090411100a121a0baa1bffab, 128'h01000302090411100a121a0baa1bffab, 128'hffffffffffffffffffffffffffffffff);
   cn = 1453; testcase_vvv(i16x8_le_u,              128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'hffffffffffff00000000000000000000);
   cn = 1454; testcase_vvv(i16x8_le_u,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000);
   cn = 1455; testcase_vvv(i16x8_le_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1456; testcase_vvv(i16x8_le_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1457; testcase_vvv(i16x8_le_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1458; testcase_vvv(i16x8_le_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1459; testcase_vvv(i16x8_le_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1460; testcase_vvv(i16x8_le_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);

   cn = 1461; testcase_vvv(i16x8_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1462; testcase_vvv(i16x8_le_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1463; testcase_vvv(i16x8_le_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1465; testcase_vvv(i16x8_le_s,              128'hff80ff80000000000001000100ff00ff, 128'h808080800000000001010101ffffffff, 128'h00000000ffffffffffffffff00000000);
   cn = 1466; testcase_vvv(i16x8_le_s,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1468; testcase_vvv(i16x8_le_s,              128'h81808382fefd00ff01007f02fd80fffe, 128'h81808382fefd00ff01007f02fd80fffe, 128'hffffffffffffffffffffffffffffffff);
   cn = 1470; testcase_vvv(i16x8_le_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1471; testcase_vvv(i16x8_le_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1472; testcase_vvv(i16x8_le_s,              128'h8000fffeffff0000000000010002ffff, 128'h8000fffeffff0000000000010002ffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1473; testcase_vvv(i16x8_le_s,              128'h80008001fffeffff0000ffff80018000, 128'h80008001ffff0000fffffffe80018000, 128'hffffffffffffffff00000000ffffffff);
   cn = 1474; testcase_vvv(i16x8_le_s,              128'h80008001800280038004800580068007, 128'h80078006800580048003800280018000, 128'hffffffffffffffff0000000000000000);
   cn = 1475; testcase_vvv(i16x8_le_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1476; testcase_vvv(i16x8_le_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1477; testcase_vvv(i16x8_le_s,              128'h30393039303930393039303930393039, 128'h30393039303930393039303930393039, 128'hffffffffffffffffffffffffffffffff);
   cn = 1478; testcase_vvv(i16x8_le_s,              128'h12341234123412341234123412341234, 128'h12341234123412341234123412341234, 128'hffffffffffffffffffffffffffffffff);
   cn = 1479; testcase_vvv(i16x8_le_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1480; testcase_vvv(i16x8_le_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1481; testcase_vvv(i16x8_le_s,              128'h01000302090411100a121a0baa1bffab, 128'h01000302090411100a121a0baa1bffab, 128'hffffffffffffffffffffffffffffffff);
   cn = 1484; testcase_vvv(i16x8_le_s,              128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'h00000000ffff0000000000000000ffff);
   cn = 1485; testcase_vvv(i16x8_le_s,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'h0000000000000000ffffffffffffffff);
   cn = 1486; testcase_vvv(i16x8_le_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1487; testcase_vvv(i16x8_le_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1488; testcase_vvv(i16x8_le_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1489; testcase_vvv(i16x8_le_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1490; testcase_vvv(i16x8_le_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1491; testcase_vvv(i16x8_le_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);

   cn = 1495; testcase_vvv(i16x8_gt_u,              128'hff80ff80000000000001000100ff00ff, 128'h808080800000000001010101ffffffff, 128'hffffffff000000000000000000000000);
   cn = 1496; testcase_vvv(i16x8_gt_u,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1498; testcase_vvv(i16x8_gt_u,              128'h81808382fefd00ff01007f02fd80fffe, 128'h81808382fefd00ff01007f02fd80fffe, 128'h00000000000000000000000000000000);
   cn = 1500; testcase_vvv(i16x8_gt_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1501; testcase_vvv(i16x8_gt_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1502; testcase_vvv(i16x8_gt_u,              128'h80008001fffeffff0000ffff80018000, 128'h80008001ffff0000fffffffe80018000, 128'h000000000000ffff0000ffff00000000);
   cn = 1503; testcase_vvv(i16x8_gt_u,              128'h80008001800280038004800580068007, 128'h80078006800580048003800280018000, 128'h0000000000000000ffffffffffffffff);
   cn = 1504; testcase_vvv(i16x8_gt_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1505; testcase_vvv(i16x8_gt_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1506; testcase_vvv(i16x8_gt_u,              128'h30393039303930393039303930393039, 128'h30393039303930393039303930393039, 128'h00000000000000000000000000000000);
   cn = 1507; testcase_vvv(i16x8_gt_u,              128'h12341234123412341234123412341234, 128'h12341234123412341234123412341234, 128'h00000000000000000000000000000000);
   cn = 1508; testcase_vvv(i16x8_gt_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1509; testcase_vvv(i16x8_gt_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h00000000000000000000000000000000);
   cn = 1510; testcase_vvv(i16x8_gt_u,              128'h01000302090411100a121a0baa1bffab, 128'h01000302090411100a121a0baa1bffab, 128'h00000000000000000000000000000000);
   cn = 1513; testcase_vvv(i16x8_gt_u,              128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'h000000000000ffffffffffffffffffff);
   cn = 1514; testcase_vvv(i16x8_gt_u,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'h0000000000000000ffffffffffffffff);
   cn = 1515; testcase_vvv(i16x8_gt_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1516; testcase_vvv(i16x8_gt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1517; testcase_vvv(i16x8_gt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1518; testcase_vvv(i16x8_gt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);

   cn = 1519; testcase_vvv(i16x8_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1520; testcase_vvv(i16x8_gt_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1521; testcase_vvv(i16x8_gt_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1522; testcase_vvv(i16x8_gt_s,              128'hffff_ffff_0000_0000_0001_0001_8000_8000, 128'h0000_ffff_0000_0000_0000_0001_0000_8000, 128'h0000_0000_0000_0000_ffff_0000_0000_0000);
   cn = 1523; testcase_vvv(i16x8_gt_s,              128'hff80_ff80_0000_0000_0001_0001_00ff_00ff, 128'h8080_8080_0000_0000_0101_0101_ffff_ffff, 128'hffff_ffff_0000_0000_0000_0000_ffff_ffff);
   cn = 1524; testcase_vvv(i16x8_gt_s,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1526; testcase_vvv(i16x8_gt_s,              128'h81808382fefd00ff01007f02fd80fffe, 128'h81808382fefd00ff01007f02fd80fffe, 128'h00000000000000000000000000000000);
   cn = 1527; testcase_vvv(i16x8_gt_s,              128'h81808382fefd00ff01007f02fd80fffe, 128'h80818283fdfeff000001027f80fdfeff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1528; testcase_vvv(i16x8_gt_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1529; testcase_vvv(i16x8_gt_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1530; testcase_vvv(i16x8_gt_s,              128'h8000fffeffff0000000000010002ffff, 128'h8000fffeffff0000000000010002ffff, 128'h00000000000000000000000000000000);
   cn = 1531; testcase_vvv(i16x8_gt_s,              128'h80008001fffeffff0000ffff80018000, 128'h80008001ffff0000fffffffe80018000, 128'h0000000000000000ffffffff00000000);
   cn = 1532; testcase_vvv(i16x8_gt_s,              128'h80008001800280038004800580068007, 128'h80078006800580048003800280018000, 128'h0000000000000000ffffffffffffffff);
   cn = 1533; testcase_vvv(i16x8_gt_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn = 1534; testcase_vvv(i16x8_gt_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn = 1535; testcase_vvv(i16x8_gt_s,              128'h30393039303930393039303930393039, 128'h30393039303930393039303930393039, 128'h00000000000000000000000000000000);
   cn = 1536; testcase_vvv(i16x8_gt_s,              128'h12341234123412341234123412341234, 128'h12341234123412341234123412341234, 128'h00000000000000000000000000000000);
   cn = 1537; testcase_vvv(i16x8_gt_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1538; testcase_vvv(i16x8_gt_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h00000000000000000000000000000000);
   cn = 1539; testcase_vvv(i16x8_gt_s,              128'h01000302090411100a121a0baa1bffab, 128'h01000302090411100a121a0baa1bffab, 128'h00000000000000000000000000000000);
   cn = 1540; testcase_vvv(i16x8_gt_s,              128'h0100_0302_0504_0706_0908_0b0a_0d0c_0f0e,128'h0302_0100_0706_0504_0b0a_0908_0f0e_0d0c,128'h0000_ffff_0000_ffff_0000_ffff_0000_ffff);
   cn = 1541; testcase_vvv(i16x8_gt_s,              128'h010003020504070609080b0a0d0c0f0e, 128'h000102030405060708090a0b0c0d0e0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1542; testcase_vvv(i16x8_gt_s,              128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'hffffffff0000ffffffffffffffff0000);
   cn = 1543; testcase_vvv(i16x8_gt_s,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000);
   cn = 1544; testcase_vvv(i16x8_gt_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1545; testcase_vvv(i16x8_gt_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1546; testcase_vvv(i16x8_gt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1547; testcase_vvv(i16x8_gt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1548; testcase_vvv(i16x8_gt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1549; testcase_vvv(i16x8_gt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);

   cn = 1550; testcase_vvv(i16x8_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1551; testcase_vvv(i16x8_ge_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1552; testcase_vvv(i16x8_ge_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1553; testcase_vvv(i16x8_ge_u,              128'hffffffff000000000001000180008000, 128'hffffff800000000000000001000000ff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1554; testcase_vvv(i16x8_ge_u,              128'hff80ff80000000000001000100ff00ff, 128'h808080800000000001010101ffffffff, 128'hffffffffffffffff0000000000000000);
   cn = 1555; testcase_vvv(i16x8_ge_u,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1556; testcase_vvv(i16x8_ge_u,              128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h55555555555555555555555555555555, 128'hffffffffffffffffffffffffffffffff);
   cn = 1557; testcase_vvv(i16x8_ge_u,              128'h81808382fefd00ff01007f02fd80fffe, 128'h8382818000fffefd7f020100fffefd80, 128'h0000ffffffff00000000ffff0000ffff);
   cn = 1558; testcase_vvv(i16x8_ge_u,              128'h81808382fefd00ff01007f02fd80fffe, 128'h81808382fefd00ff01007f02fd80fffe, 128'hffffffffffffffffffffffffffffffff);
   cn = 1559; testcase_vvv(i16x8_ge_u,              128'h81808382fefd00ff01007f02fd80fffe, 128'h80818283fdfeff000001027f80fdfeff, 128'hffffffffffff0000ffffffffffffffff);
   cn = 1560; testcase_vvv(i16x8_ge_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1561; testcase_vvv(i16x8_ge_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1562; testcase_vvv(i16x8_ge_u,              128'h8000fffeffff0000000000010002ffff, 128'h8000fffeffff0000000000010002ffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1563; testcase_vvv(i16x8_ge_u,              128'h80008001fffeffff0000ffff80018000, 128'h80008001ffff0000fffffffe80018000, 128'hffffffff0000ffff0000ffffffffffff);
   cn = 1564; testcase_vvv(i16x8_ge_u,              128'h80008001800280038004800580068007, 128'h80078006800580048003800280018000, 128'h0000000000000000ffffffffffffffff);
   cn = 1565; testcase_vvv(i16x8_ge_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1566; testcase_vvv(i16x8_ge_u,              128'h30393039303930393039303930393039, 128'h30393039303930393039303930393039, 128'hffffffffffffffffffffffffffffffff);
   cn = 1567; testcase_vvv(i16x8_ge_u,              128'h12341234123412341234123412341234, 128'h12341234123412341234123412341234, 128'hffffffffffffffffffffffffffffffff);
   cn = 1568; testcase_vvv(i16x8_ge_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1569; testcase_vvv(i16x8_ge_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1570; testcase_vvv(i16x8_ge_u,              128'h01000302090411100a121a0baa1bffab, 128'h01000302090411100a121a0baa1bffab, 128'hffffffffffffffffffffffffffffffff);
   cn = 1571; testcase_vvv(i16x8_ge_u,              128'h0100_0302_0504_0706_0908_0b0a_0d0c_0f0e, 128'h0302_0100_0706_0504_0b0a_0908_0f0e_0d0c, 128'h0000_ffff_0000_ffff_0000_ffff_0000_ffff);
   cn = 1572; testcase_vvv(i16x8_ge_u,              128'h010003020504070609080b0a0d0c0f0e, 128'h000102030405060708090a0b0c0d0e0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1573; testcase_vvv(i16x8_ge_u,              128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'h000000000000ffffffffffffffffffff);
   cn = 1574; testcase_vvv(i16x8_ge_u,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'h0000000000000000ffffffffffffffff);
   cn = 1575; testcase_vvv(i16x8_ge_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1576; testcase_vvv(i16x8_ge_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1577; testcase_vvv(i16x8_ge_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1578; testcase_vvv(i16x8_ge_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1579; testcase_vvv(i16x8_ge_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1580; testcase_vvv(i16x8_ge_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1581; testcase_vvv(i16x8_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1582; testcase_vvv(i16x8_ge_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1583; testcase_vvv(i16x8_ge_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1584; testcase_vvv(i16x8_ge_s,              128'hffff_ffff_0000_0000_0001_0001_8000_8000, 128'h0000_ffff_0000_0000_0000_0001_0000_8000, 128'h0000_ffff_ffff_ffff_ffff_ffff_0000_ffff);
   cn = 1585; testcase_vvv(i16x8_ge_s,              128'hff80ff80000000000001000100ff00ff, 128'h808080800000000001010101ffffffff, 128'hffffffffffffffff00000000ffffffff);
   cn = 1586; testcase_vvv(i16x8_ge_s,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1587; testcase_vvv(i16x8_ge_s,              128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h55555555555555555555555555555555, 128'h00000000000000000000000000000000);
   cn = 1588; testcase_vvv(i16x8_ge_s,              128'h8180_8382_fefd_00ff_0100_7f02_fd80_fffe, 128'h8382_8180_00ff_fefd_7f02_0100_fffe_fd80, 128'h0000_ffff_0000_ffff_0000_ffff_0000_ffff);
   cn = 1589; testcase_vvv(i16x8_ge_s,              128'h81808382fefd00ff01007f02fd80fffe, 128'h81808382fefd00ff01007f02fd80fffe, 128'hffffffffffffffffffffffffffffffff);
   cn = 1590; testcase_vvv(i16x8_ge_s,              128'h81808382fefd00ff01007f02fd80fffe, 128'h80818283fdfeff000001027f80fdfeff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1591; testcase_vvv(i16x8_ge_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1592; testcase_vvv(i16x8_ge_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1593; testcase_vvv(i16x8_ge_s,              128'h8000fffeffff0000000000010002ffff, 128'h8000fffeffff0000000000010002ffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1594; testcase_vvv(i16x8_ge_s,              128'h80008001fffeffff0000ffff80018000, 128'h80008001ffff0000fffffffe80018000, 128'hffffffff00000000ffffffffffffffff);
   cn = 1595; testcase_vvv(i16x8_ge_s,              128'h80008001800280038004800580068007, 128'h80078006800580048003800280018000, 128'h0000000000000000ffffffffffffffff);
   cn = 1596; testcase_vvv(i16x8_ge_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn = 1597; testcase_vvv(i16x8_ge_s,              128'h30393039303930393039303930393039, 128'h30393039303930393039303930393039, 128'hffffffffffffffffffffffffffffffff);
   cn = 1598; testcase_vvv(i16x8_ge_s,              128'h12341234123412341234123412341234, 128'h12341234123412341234123412341234, 128'hffffffffffffffffffffffffffffffff);
   cn = 1599; testcase_vvv(i16x8_ge_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1600; testcase_vvv(i16x8_ge_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1601; testcase_vvv(i16x8_ge_s,              128'h01000302090411100a121a0baa1bffab, 128'h01000302090411100a121a0baa1bffab, 128'hffffffffffffffffffffffffffffffff);
   cn = 1602; testcase_vvv(i16x8_ge_s,              128'h0100_0302_0504_0706_0908_0b0a_0d0c_0f0e, 128'h0302_0100_0706_0504_0b0a_0908_0f0e_0d0c, 128'h0000_ffff_0000_ffff_0000_ffff_0000_ffff);
   cn = 1603; testcase_vvv(i16x8_ge_s,              128'h010003020504070609080b0a0d0c0f0e, 128'h000102030405060708090a0b0c0d0e0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1604; testcase_vvv(i16x8_ge_s,              128'h0001020304091011120a0b1a1baaabff, 128'hffabaa1b1a0b0a121110090403020100, 128'hffffffff0000ffffffffffffffff0000);
   cn = 1605; testcase_vvv(i16x8_ge_s,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000);
   cn = 1606; testcase_vvv(i16x8_ge_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1607; testcase_vvv(i16x8_ge_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1608; testcase_vvv(i16x8_ge_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);

   cn = 1678; testcase_vvv(i32x4_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1679; testcase_vvv(i32x4_ne,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1690; testcase_vvv(i32x4_ne,                128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1691; testcase_vvv(i32x4_ne,                128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h00000000000000000000000000000000);
   cn = 1692; testcase_vvv(i32x4_ne,                128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1693; testcase_vvv(i32x4_ne,                128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1694; testcase_vvv(i32x4_ne,                128'h03020100111009041a0b0a12ffabaa1b, 128'h03020100111009041a0b0a12ffabaa1b, 128'h00000000000000000000000000000000);
   cn = 1695; testcase_vvv(i32x4_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1696; testcase_vvv(i32x4_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1697; testcase_vvv(i32x4_ne,                128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1698; testcase_vvv(i32x4_ne,                128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1699; testcase_vvv(i32x4_ne,                128'h8382818000fffefd7f020100fffefd80, 128'h8382818000fffefd7f020100fffefd80, 128'h00000000000000000000000000000000);
   cn = 1700; testcase_vvv(i32x4_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1701; testcase_vvv(i32x4_ne,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1702; testcase_vvv(i32x4_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1703; testcase_vvv(i32x4_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1704; testcase_vvv(i32x4_ne,                128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1705; testcase_vvv(i32x4_ne,                128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1706; testcase_vvv(i32x4_ne,                128'h80000001ffffffff00000000ffffffff, 128'h80000001ffffffff00000000ffffffff, 128'h00000000000000000000000000000000);
   cn = 1707; testcase_vvv(i32x4_ne,                128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1708; testcase_vvv(i32x4_ne,                128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1709; testcase_vvv(i32x4_ne,                128'h02030001101104090b1a120aabff1baa, 128'haa1bffab0a121a0b0904111001000302, 128'hffffffffffffffffffffffffffffffff);
   cn = 1710; testcase_vvv(i32x4_ne,                128'h80018000800380028005800480078006, 128'h80078006800580048003800280018000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1711; testcase_vvv(i32x4_ne,                128'h800000007fffffff00000000ffffffff, 128'h8000000080000001ffffffff00000000, 128'h00000000ffffffffffffffffffffffff);
   cn = 1712; testcase_vvv(i32x4_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1713; testcase_vvv(i32x4_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1714; testcase_vvv(i32x4_ne,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1715; testcase_vvv(i32x4_ne,                128'h03020100_07060504_0b0a0908_0f0e0d0c, 128'h00010203_04050607_08090a0b_0c0d0e0f, 128'hffffffff_ffffffff_ffffffff_ffffffff);
   cn = 1716; testcase_vvv(i32x4_ne,                128'h83828180_00fffefd_7f020100_fffefd80, 128'h80818283_fdfeff00_0001027f_80fdfeff, 128'hffffffff_ffffffff_ffffffff_ffffffff);
   cn = 1717; testcase_vvv(i32x4_ne,                128'hff80ff800000000000000001ffffffff, 128'h808080800000000001010101ffffffff, 128'hffffffff00000000ffffffff00000000);
   cn = 1718; testcase_vvv(i32x4_ne,                128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn = 1719; testcase_vvv(i32x4_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1720; testcase_vvv(i32x4_ne,                128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1721; testcase_vvv(i32x4_ne,                128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1722; testcase_vvv(i32x4_ne,                128'h03020100_07060504_0b0a0908_0f0e0d0c, 128'h01000302_05040706_09080b0a_0d0c0f0e, 128'hffffffff_ffffffff_ffffffff_ffffffff);
   cn = 1723; testcase_vvv(i32x4_ne,                128'h83828180_00fffefd_7f020100_fffefd80, 128'h81808382_fefd00ff_01007f02_fd80fffe, 128'hffffffff_ffffffff_ffffffff_ffffffff);
   cn = 1724; testcase_vvv(i32x4_ne,                128'hffffff800000000000000001000000ff, 128'hff80ff80000000000001000100ff00ff, 128'hffffffff00000000ffffffffffffffff);
   cn = 1725; testcase_vvv(i32x4_ne,                128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h55555555555555555555555555555555, 128'hffffffffffffffffffffffffffffffff);
   cn = 1726; testcase_vvv(i32x4_ne,                128'h075bcd15075bcd15075bcd15075bcd15, 128'h075bcd15075bcd15075bcd15075bcd15, 128'h00000000000000000000000000000000);
   cn = 1727; testcase_vvv(i32x4_ne,                128'h12345678123456781234567812345678, 128'h12345678123456781234567812345678, 128'h00000000000000000000000000000000);

   cn = 1808; testcase_vvv(i32x4_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1809; testcase_vvv(i32x4_le_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1810; testcase_vvv(i32x4_le_s,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1811; testcase_vvv(i32x4_le_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1812; testcase_vvv(i32x4_le_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1813; testcase_vvv(i32x4_le_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1814; testcase_vvv(i32x4_le_s,              128'h03020100111009041a0b0a12ffabaa1b, 128'h03020100111009041a0b0a12ffabaa1b, 128'hffffffffffffffffffffffffffffffff);
   cn = 1815; testcase_vvv(i32x4_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1816; testcase_vvv(i32x4_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1817; testcase_vvv(i32x4_le_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1818; testcase_vvv(i32x4_le_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1819; testcase_vvv(i32x4_le_s,              128'h8382818000fffefd7f020100fffefd80, 128'h8382818000fffefd7f020100fffefd80, 128'hffffffffffffffffffffffffffffffff);
   cn = 1820; testcase_vvv(i32x4_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1821; testcase_vvv(i32x4_le_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1822; testcase_vvv(i32x4_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1823; testcase_vvv(i32x4_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1824; testcase_vvv(i32x4_le_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1825; testcase_vvv(i32x4_le_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1826; testcase_vvv(i32x4_le_s,              128'h80000001ffffffff00000000ffffffff, 128'h80000001ffffffff00000000ffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1827; testcase_vvv(i32x4_le_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1828; testcase_vvv(i32x4_le_s,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'h0000000000000000ffffffffffffffff);
   cn = 1829; testcase_vvv(i32x4_le_s,              128'h02030001101104090b1a120aabff1baa, 128'haa1bffab0a121a0b0904111001000302, 128'h000000000000000000000000ffffffff);
   cn = 1830; testcase_vvv(i32x4_le_s,              128'h80018000800380028005800480078006, 128'h80078006800580048003800280018000, 128'hffffffffffffffff0000000000000000);
   cn = 1831; testcase_vvv(i32x4_le_s,              128'h800000007fffffff00000000ffffffff, 128'h8000000080000001ffffffff00000000, 128'hffffffff0000000000000000ffffffff);
   cn = 1832; testcase_vvv(i32x4_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1833; testcase_vvv(i32x4_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1834; testcase_vvv(i32x4_le_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1835; testcase_vvv(i32x4_le_s,              128'h03020100_07060504_0b0a0908_0f0e0d0c, 128'h00010203_04050607_08090a0b_0c0d0e0f, 128'h00000000_00000000_00000000_00000000);
   cn = 1836; testcase_vvv(i32x4_le_s,              128'h83828180_00fffefd_7f020100_fffefd80, 128'h80818283_fdfeff00_0001027f_80fdfeff, 128'h00000000_00000000_00000000_00000000);
   cn = 1837; testcase_vvv(i32x4_le_s,              128'hff80ff800000000000000001ffffffff, 128'h808080800000000001010101ffffffff, 128'h00000000ffffffffffffffffffffffff);
   cn = 1838; testcase_vvv(i32x4_le_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1839; testcase_vvv(i32x4_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1840; testcase_vvv(i32x4_le_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1841; testcase_vvv(i32x4_le_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1842; testcase_vvv(i32x4_le_s,              128'h03020100_07060504_0b0a0908_0f0e0d0c, 128'h01000302_05040706_09080b0a_0d0c0f0e, 128'h00000000_00000000_00000000_00000000);
   cn = 1843; testcase_vvv(i32x4_le_s,              128'h83828180_00fffefd_7f020100_fffefd80, 128'h81808382_fefd00ff_01007f02_fd80fffe, 128'h00000000_00000000_00000000_00000000);
   cn = 1844; testcase_vvv(i32x4_le_s,              128'hffffff800000000000000001000000ff, 128'hff80ff80000000000001000100ff00ff, 128'h00000000ffffffffffffffffffffffff);
   cn = 1845; testcase_vvv(i32x4_le_s,              128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h55555555555555555555555555555555, 128'hffffffffffffffffffffffffffffffff);
   cn = 1846; testcase_vvv(i32x4_le_s,              128'h075bcd15075bcd15075bcd15075bcd15, 128'h075bcd15075bcd15075bcd15075bcd15, 128'hffffffffffffffffffffffffffffffff);
   cn = 1847; testcase_vvv(i32x4_le_s,              128'h12345678123456781234567812345678, 128'h12345678123456781234567812345678, 128'hffffffffffffffffffffffffffffffff);

   cn = 1848; testcase_vvv(i32x4_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1849; testcase_vvv(i32x4_le_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1850; testcase_vvv(i32x4_le_u,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1851; testcase_vvv(i32x4_le_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1852; testcase_vvv(i32x4_le_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1853; testcase_vvv(i32x4_le_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1854; testcase_vvv(i32x4_le_u,              128'h03020100111009041a0b0a12ffabaa1b, 128'h03020100111009041a0b0a12ffabaa1b, 128'hffffffffffffffffffffffffffffffff);
   cn = 1855; testcase_vvv(i32x4_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1856; testcase_vvv(i32x4_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1857; testcase_vvv(i32x4_le_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1858; testcase_vvv(i32x4_le_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1859; testcase_vvv(i32x4_le_u,              128'h8382818000fffefd7f020100fffefd80, 128'h8382818000fffefd7f020100fffefd80, 128'hffffffffffffffffffffffffffffffff);
   cn = 1860; testcase_vvv(i32x4_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1861; testcase_vvv(i32x4_le_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1862; testcase_vvv(i32x4_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1863; testcase_vvv(i32x4_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1864; testcase_vvv(i32x4_le_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1865; testcase_vvv(i32x4_le_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1866; testcase_vvv(i32x4_le_u,              128'h80000001ffffffff00000000ffffffff, 128'h80000001ffffffff00000000ffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1867; testcase_vvv(i32x4_le_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1868; testcase_vvv(i32x4_le_u,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000);
   cn = 1869; testcase_vvv(i32x4_le_u,              128'h02030001101104090b1a120aabff1baa, 128'haa1bffab0a121a0b0904111001000302, 128'hffffffff000000000000000000000000);
   cn = 1870; testcase_vvv(i32x4_le_u,              128'h80018000800380028005800480078006, 128'h80078006800580048003800280018000, 128'hffffffffffffffff0000000000000000);
   cn = 1871; testcase_vvv(i32x4_le_u,              128'h800000007fffffff00000000ffffffff, 128'h8000000080000001ffffffff00000000, 128'hffffffffffffffffffffffff00000000);
   cn = 1872; testcase_vvv(i32x4_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1873; testcase_vvv(i32x4_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1874; testcase_vvv(i32x4_le_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1875; testcase_vvv(i32x4_le_u,              128'h03020100_07060504_0b0a0908_0f0e0d0c, 128'h00010203_04050607_08090a0b_0c0d0e0f, 128'h00000000_00000000_00000000_00000000);
   cn = 1876; testcase_vvv(i32x4_le_u,              128'h83828180_00fffefd_7f020100_fffefd80, 128'h80818283_fdfeff00_0001027f_80fdfeff, 128'h00000000_ffffffff_00000000_00000000);
   cn = 1877; testcase_vvv(i32x4_le_u,              128'hff80ff800000000000000001ffffffff, 128'h808080800000000001010101ffffffff, 128'h00000000ffffffffffffffffffffffff);
   cn = 1878; testcase_vvv(i32x4_le_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn = 1879; testcase_vvv(i32x4_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1880; testcase_vvv(i32x4_le_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1881; testcase_vvv(i32x4_le_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1882; testcase_vvv(i32x4_le_u,              128'h03020100_07060504_0b0a0908_0f0e0d0c, 128'h01000302_05040706_09080b0a_0d0c0f0e, 128'h00000000_00000000_00000000_00000000);
   cn = 1883; testcase_vvv(i32x4_le_u,              128'h83828180_00fffefd_7f020100_fffefd80, 128'h81808382_fefd00ff_01007f02_fd80fffe, 128'h00000000_ffffffff_00000000_00000000);
   cn = 1884; testcase_vvv(i32x4_le_u,              128'hffffff800000000000000001000000ff, 128'hff80ff80000000000001000100ff00ff, 128'h00000000ffffffffffffffffffffffff);
   cn = 1885; testcase_vvv(i32x4_le_u,              128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h55555555555555555555555555555555, 128'h00000000000000000000000000000000);
   cn = 1886; testcase_vvv(i32x4_le_u,              128'h075bcd15075bcd15075bcd15075bcd15, 128'h075bcd15075bcd15075bcd15075bcd15, 128'hffffffffffffffffffffffffffffffff);
   cn = 1887; testcase_vvv(i32x4_le_u,              128'h90abcdef90abcdef90abcdef90abcdef, 128'h90abcdef90abcdef90abcdef90abcdef, 128'hffffffffffffffffffffffffffffffff);

   cn = 1888; testcase_vvv(i32x4_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1889; testcase_vvv(i32x4_gt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1890; testcase_vvv(i32x4_gt_s,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1891; testcase_vvv(i32x4_gt_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h00000000000000000000000000000000);
   cn = 1892; testcase_vvv(i32x4_gt_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1893; testcase_vvv(i32x4_gt_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1894; testcase_vvv(i32x4_gt_s,              128'h03020100111009041a0b0a12ffabaa1b, 128'h03020100111009041a0b0a12ffabaa1b, 128'h00000000000000000000000000000000);
   cn = 1895; testcase_vvv(i32x4_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1896; testcase_vvv(i32x4_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1897; testcase_vvv(i32x4_gt_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1898; testcase_vvv(i32x4_gt_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1899; testcase_vvv(i32x4_gt_s,              128'h8382818000fffefd7f020100fffefd80, 128'h8382818000fffefd7f020100fffefd80, 128'h00000000000000000000000000000000);
   cn = 1900; testcase_vvv(i32x4_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1901; testcase_vvv(i32x4_gt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1902; testcase_vvv(i32x4_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1903; testcase_vvv(i32x4_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1904; testcase_vvv(i32x4_gt_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1905; testcase_vvv(i32x4_gt_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1906; testcase_vvv(i32x4_gt_s,              128'h80000001ffffffff00000000ffffffff, 128'h80000001ffffffff00000000ffffffff, 128'h00000000000000000000000000000000);
   cn = 1907; testcase_vvv(i32x4_gt_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1908; testcase_vvv(i32x4_gt_s,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000);
   cn = 1909; testcase_vvv(i32x4_gt_s,              128'h02030001101104090b1a120aabff1baa, 128'haa1bffab0a121a0b0904111001000302, 128'hffffffffffffffffffffffff00000000);
   cn = 1910; testcase_vvv(i32x4_gt_s,              128'h80018000800380028005800480078006, 128'h80078006800580048003800280018000, 128'h0000000000000000ffffffffffffffff);
   cn = 1911; testcase_vvv(i32x4_gt_s,              128'h800000007fffffff00000000ffffffff, 128'h8000000080000001ffffffff00000000, 128'h00000000ffffffffffffffff00000000);
   cn = 1912; testcase_vvv(i32x4_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1913; testcase_vvv(i32x4_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1914; testcase_vvv(i32x4_gt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1915; testcase_vvv(i32x4_gt_s,              128'h03020100_07060504_0b0a0908_0f0e0d0c, 128'h00010203_04050607_08090a0b_0c0d0e0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1916; testcase_vvv(i32x4_gt_s,              128'h83828180_00fffefd_7f020100_fffefd80, 128'h80818283_fdfeff00_0001027f_80fdfeff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1917; testcase_vvv(i32x4_gt_s,              128'hff80ff800000000000000001ffffffff, 128'h808080800000000001010101ffffffff, 128'hffffffff000000000000000000000000);
   cn = 1918; testcase_vvv(i32x4_gt_s,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'hffffffffffffffffffffffffffffffff);
   cn = 1919; testcase_vvv(i32x4_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1920; testcase_vvv(i32x4_gt_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1921; testcase_vvv(i32x4_gt_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1922; testcase_vvv(i32x4_gt_s,              128'h03020100_07060504_0b0a0908_0f0e0d0c, 128'h01000302_05040706_09080b0a_0d0c0f0e, 128'hffffffff_ffffffff_ffffffff_ffffffff);
   cn = 1923; testcase_vvv(i32x4_gt_s,              128'h83828180_00fffefd_7f020100_fffefd80, 128'h81808382_fefd00ff_01007f02_fd80fffe, 128'hffffffff_ffffffff_ffffffff_ffffffff);
   cn = 1924; testcase_vvv(i32x4_gt_s,              128'h0000ffff000000000000000100008000, 128'hffffffff000000000001000180008000, 128'hffffffff0000000000000000ffffffff);
   cn = 1925; testcase_vvv(i32x4_gt_s,              128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h55555555555555555555555555555555, 128'h00000000000000000000000000000000);
   cn = 1926; testcase_vvv(i32x4_gt_s,              128'h075bcd15075bcd15075bcd15075bcd15, 128'h075bcd15075bcd15075bcd15075bcd15, 128'h00000000000000000000000000000000);
   cn = 1927; testcase_vvv(i32x4_gt_s,              128'h90abcdef90abcdef90abcdef90abcdef, 128'h90abcdef90abcdef90abcdef90abcdef, 128'h00000000000000000000000000000000);

   cn = 1928; testcase_vvv(i32x4_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1929; testcase_vvv(i32x4_gt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1930; testcase_vvv(i32x4_gt_u,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1931; testcase_vvv(i32x4_gt_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h00000000000000000000000000000000);
   cn = 1932; testcase_vvv(i32x4_gt_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1933; testcase_vvv(i32x4_gt_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1934; testcase_vvv(i32x4_gt_u,              128'h03020100111009041a0b0a12ffabaa1b, 128'h03020100111009041a0b0a12ffabaa1b, 128'h00000000000000000000000000000000);
   cn = 1935; testcase_vvv(i32x4_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1936; testcase_vvv(i32x4_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1937; testcase_vvv(i32x4_gt_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1938; testcase_vvv(i32x4_gt_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 1939; testcase_vvv(i32x4_gt_u,              128'h8382818000fffefd7f020100fffefd80, 128'h8382818000fffefd7f020100fffefd80, 128'h00000000000000000000000000000000);
   cn = 1940; testcase_vvv(i32x4_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1941; testcase_vvv(i32x4_gt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1942; testcase_vvv(i32x4_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1943; testcase_vvv(i32x4_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1944; testcase_vvv(i32x4_gt_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'h00000000000000000000000000000000);
   cn = 1945; testcase_vvv(i32x4_gt_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1946; testcase_vvv(i32x4_gt_u,              128'h80000001ffffffff00000000ffffffff, 128'h80000001ffffffff00000000ffffffff, 128'h00000000000000000000000000000000);
   cn = 1947; testcase_vvv(i32x4_gt_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 1948; testcase_vvv(i32x4_gt_u,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'h0000000000000000ffffffffffffffff);
   cn = 1949; testcase_vvv(i32x4_gt_u,              128'h02030001101104090b1a120aabff1baa, 128'haa1bffab0a121a0b0904111001000302, 128'h00000000ffffffffffffffffffffffff);
   cn = 1950; testcase_vvv(i32x4_gt_u,              128'h80018000800380028005800480078006, 128'h80078006800580048003800280018000, 128'h0000000000000000ffffffffffffffff);
   cn = 1951; testcase_vvv(i32x4_gt_u,              128'h800000007fffffff00000000ffffffff, 128'h8000000080000001ffffffff00000000, 128'h000000000000000000000000ffffffff);
   cn = 1952; testcase_vvv(i32x4_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1953; testcase_vvv(i32x4_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1954; testcase_vvv(i32x4_gt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1955; testcase_vvv(i32x4_gt_u,              128'h03020100_07060504_0b0a0908_0f0e0d0c, 128'h00010203_04050607_08090a0b_0c0d0e0f, 128'hffffffff_ffffffff_ffffffff_ffffffff);
   cn = 1956; testcase_vvv(i32x4_gt_u,              128'h83828180_00fffefd_7f020100_fffefd80, 128'h80818283_fdfeff00_0001027f_80fdfeff, 128'hffffffff_00000000_ffffffff_ffffffff);
   cn = 1957; testcase_vvv(i32x4_gt_u,              128'hff80ff800000000000000001ffffffff, 128'h808080800000000001010101ffffffff, 128'hffffffff000000000000000000000000);
   cn = 1958; testcase_vvv(i32x4_gt_u,              128'h55555555555555555555555555555555, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h00000000000000000000000000000000);
   cn = 1959; testcase_vvv(i32x4_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1960; testcase_vvv(i32x4_gt_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 1961; testcase_vvv(i32x4_gt_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 1962; testcase_vvv(i32x4_gt_u,              128'h03020100_07060504_0b0a0908_0f0e0d0c, 128'h01000302_05040706_09080b0a_0d0c0f0e, 128'hffffffff_ffffffff_ffffffff_ffffffff);
   cn = 1963; testcase_vvv(i32x4_gt_u,              128'h83828180_00fffefd_7f020100_fffefd80, 128'h81808382_fefd00ff_01007f02_fd80fffe, 128'hffffffff_00000000_ffffffff_ffffffff);
   cn = 1964; testcase_vvv(i32x4_gt_u,              128'hffffff800000000000000001000000ff, 128'hff80ff80000000000001000100ff00ff, 128'hffffffff000000000000000000000000);
   cn = 1965; testcase_vvv(i32x4_gt_u,              128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h55555555555555555555555555555555, 128'hffffffffffffffffffffffffffffffff);
   cn = 1966; testcase_vvv(i32x4_gt_u,              128'h075bcd15075bcd15075bcd15075bcd15, 128'h075bcd15075bcd15075bcd15075bcd15, 128'h00000000000000000000000000000000);
   cn = 1967; testcase_vvv(i32x4_gt_u,              128'h12345678123456781234567812345678, 128'h12345678123456781234567812345678, 128'h00000000000000000000000000000000);
   cn = 1968; testcase_vvv(i32x4_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1969; testcase_vvv(i32x4_ge_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1970; testcase_vvv(i32x4_ge_s,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1971; testcase_vvv(i32x4_ge_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1972; testcase_vvv(i32x4_ge_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1973; testcase_vvv(i32x4_ge_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1974; testcase_vvv(i32x4_ge_s,              128'h03020100111009041a0b0a12ffabaa1b, 128'h03020100111009041a0b0a12ffabaa1b, 128'hffffffffffffffffffffffffffffffff);
   cn = 1975; testcase_vvv(i32x4_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1976; testcase_vvv(i32x4_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1977; testcase_vvv(i32x4_ge_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1978; testcase_vvv(i32x4_ge_s,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 1979; testcase_vvv(i32x4_ge_s,              128'h8382818000fffefd7f020100fffefd80, 128'h8382818000fffefd7f020100fffefd80, 128'hffffffffffffffffffffffffffffffff);
   cn = 1980; testcase_vvv(i32x4_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1981; testcase_vvv(i32x4_ge_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1982; testcase_vvv(i32x4_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1983; testcase_vvv(i32x4_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1984; testcase_vvv(i32x4_ge_s,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1985; testcase_vvv(i32x4_ge_s,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1986; testcase_vvv(i32x4_ge_s,              128'h80000001ffffffff00000000ffffffff, 128'h80000001ffffffff00000000ffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1987; testcase_vvv(i32x4_ge_s,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 1988; testcase_vvv(i32x4_ge_s,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000);
   cn = 1989; testcase_vvv(i32x4_ge_s,              128'h02030001101104090b1a120aabff1baa, 128'haa1bffab0a121a0b0904111001000302, 128'hffffffffffffffffffffffff00000000);
   cn = 1990; testcase_vvv(i32x4_ge_s,              128'h80018000800380028005800480078006, 128'h80078006800580048003800280018000, 128'h0000000000000000ffffffffffffffff);
   cn = 1991; testcase_vvv(i32x4_ge_s,              128'h800000007fffffff00000000ffffffff, 128'h8000000080000001ffffffff00000000, 128'hffffffffffffffffffffffff00000000);
   cn = 1992; testcase_vvv(i32x4_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1993; testcase_vvv(i32x4_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1994; testcase_vvv(i32x4_ge_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 1995; testcase_vvv(i32x4_ge_s,              128'h03020100070605040b0a09080f0e0d0c, 128'h000102030405060708090a0b0c0d0e0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 1996; testcase_vvv(i32x4_ge_s,              128'h8382818000fffefd7f020100fffefd80, 128'h80818283fdfeff000001027f80fdfeff, 128'hffffffffffffffffffffffffffffffff);
   cn = 1997; testcase_vvv(i32x4_ge_s,              128'hff80ff800000000000000001ffffffff, 128'h808080800000000001010101ffffffff, 128'hffffffffffffffff00000000ffffffff);
   cn = 1998; testcase_vvv(i32x4_ge_s,              128'h55555555555555555555555555555555, 128'h55555555555555555555555555555555, 128'hffffffffffffffffffffffffffffffff);
   cn = 1999; testcase_vvv(i32x4_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2000; testcase_vvv(i32x4_ge_s,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2001; testcase_vvv(i32x4_ge_s,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2002; testcase_vvv(i32x4_ge_s,              128'h03020100070605040b0a09080f0e0d0c, 128'h010003020504070609080b0a0d0c0f0e, 128'hffffffffffffffffffffffffffffffff);
   cn = 2003; testcase_vvv(i32x4_ge_s,              128'h8382818000fffefd7f020100fffefd80, 128'h81808382fefd00ff01007f02fd80fffe, 128'hffffffffffffffffffffffffffffffff);
   cn = 2004; testcase_vvv(i32x4_ge_s,              128'h0000ffff000000000000000100008000, 128'hffffffff000000000001000180008000, 128'hffffffffffffffff00000000ffffffff);
   cn = 2005; testcase_vvv(i32x4_ge_s,              128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h55555555555555555555555555555555, 128'h00000000000000000000000000000000);
   cn = 2006; testcase_vvv(i32x4_ge_s,              128'h075bcd15075bcd15075bcd15075bcd15, 128'h075bcd15075bcd15075bcd15075bcd15, 128'hffffffffffffffffffffffffffffffff);
   cn = 2007; testcase_vvv(i32x4_ge_s,              128'h12345678123456781234567812345678, 128'h12345678123456781234567812345678, 128'hffffffffffffffffffffffffffffffff);
   cn = 2008; testcase_vvv(i32x4_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2009; testcase_vvv(i32x4_ge_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2010; testcase_vvv(i32x4_ge_u,              128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'hffffffffffffffffffffffffffffffff);
   cn = 2011; testcase_vvv(i32x4_ge_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 2012; testcase_vvv(i32x4_ge_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2013; testcase_vvv(i32x4_ge_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2014; testcase_vvv(i32x4_ge_u,              128'h03020100111009041a0b0a12ffabaa1b, 128'h03020100111009041a0b0a12ffabaa1b, 128'hffffffffffffffffffffffffffffffff);
   cn = 2015; testcase_vvv(i32x4_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2016; testcase_vvv(i32x4_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2017; testcase_vvv(i32x4_ge_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 2018; testcase_vvv(i32x4_ge_u,              128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 2019; testcase_vvv(i32x4_ge_u,              128'h8382818000fffefd7f020100fffefd80, 128'h8382818000fffefd7f020100fffefd80, 128'hffffffffffffffffffffffffffffffff);
   cn = 2020; testcase_vvv(i32x4_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2021; testcase_vvv(i32x4_ge_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2022; testcase_vvv(i32x4_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2023; testcase_vvv(i32x4_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2024; testcase_vvv(i32x4_ge_u,              128'hffffffffffffffff0000000000000000, 128'hffffffffffffffff0000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2025; testcase_vvv(i32x4_ge_u,              128'h0000000000000000ffffffffffffffff, 128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2026; testcase_vvv(i32x4_ge_u,              128'h80000001ffffffff00000000ffffffff, 128'h80000001ffffffff00000000ffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2027; testcase_vvv(i32x4_ge_u,              128'h0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0, 128'h00000000000000000000000000000000);
   cn = 2028; testcase_vvv(i32x4_ge_u,              128'h0000000000000000ffffffffffffffff, 128'hffffffffffffffff0000000000000000, 128'h0000000000000000ffffffffffffffff);
   cn = 2029; testcase_vvv(i32x4_ge_u,              128'h02030001101104090b1a120aabff1baa, 128'haa1bffab0a121a0b0904111001000302, 128'h00000000ffffffffffffffffffffffff);
   cn = 2030; testcase_vvv(i32x4_ge_u,              128'h80018000800380028005800480078006, 128'h80078006800580048003800280018000, 128'h0000000000000000ffffffffffffffff);
   cn = 2031; testcase_vvv(i32x4_ge_u,              128'h800000007fffffff00000000ffffffff, 128'h8000000080000001ffffffff00000000, 128'hffffffff0000000000000000ffffffff);
   cn = 2032; testcase_vvv(i32x4_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2033; testcase_vvv(i32x4_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2034; testcase_vvv(i32x4_ge_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2035; testcase_vvv(i32x4_ge_u,              128'h03020100070605040b0a09080f0e0d0c, 128'h000102030405060708090a0b0c0d0e0f, 128'hffffffffffffffffffffffffffffffff);
   cn = 2036; testcase_vvv(i32x4_ge_u,              128'h83828180_00fffefd_7f020100_fffefd80, 128'h80818283_fdfeff00_0001027f_80fdfeff, 128'hffffffff_00000000_ffffffff_ffffffff);
   cn = 2037; testcase_vvv(i32x4_ge_u,              128'hff80ff800000000000000001ffffffff, 128'h808080800000000001010101ffffffff, 128'hffffffffffffffff00000000ffffffff);
   cn = 2038; testcase_vvv(i32x4_ge_u,              128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h55555555555555555555555555555555, 128'hffffffffffffffffffffffffffffffff);
   cn = 2039; testcase_vvv(i32x4_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2040; testcase_vvv(i32x4_ge_u,              128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2041; testcase_vvv(i32x4_ge_u,              128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2042; testcase_vvv(i32x4_ge_u,              128'h03020100070605040b0a09080f0e0d0c, 128'h010003020504070609080b0a0d0c0f0e, 128'hffffffffffffffffffffffffffffffff);
   cn = 2043; testcase_vvv(i32x4_ge_u,              128'h83828180_00fffefd_7f020100_fffefd80, 128'h81808382_fefd00ff_01007f02_fd80fffe, 128'hffffffff_00000000_ffffffff_ffffffff);
   cn = 2044; testcase_vvv(i32x4_ge_u,              128'hffffff800000000000000001000000ff, 128'hffffffff000000000001000180008000, 128'h00000000ffffffff0000000000000000);
   cn = 2045; testcase_vvv(i32x4_ge_u,              128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'h55555555555555555555555555555555, 128'hffffffffffffffffffffffffffffffff);
   cn = 2046; testcase_vvv(i32x4_ge_u,              128'h075bcd15075bcd15075bcd15075bcd15, 128'h075bcd15075bcd15075bcd15075bcd15, 128'hffffffffffffffffffffffffffffffff);
   cn = 2047; testcase_vvv(i32x4_ge_u,              128'h12345678123456781234567812345678, 128'h12345678123456781234567812345678, 128'hffffffffffffffffffffffffffffffff);
   `endif //ALL_RELATIONALS
   `ifdef   ENABLE_NEGABSSAT
   $display(" Testing  add_sat_s");
   cn = 2136; testcase_vvv(i8x16_add_sat_s,         128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 2137; testcase_vvv(i8x16_add_sat_s,         128'h00000000000000000000000000000000, 128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101);
   cn = 2138; testcase_vvv(i8x16_add_sat_s,         128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101, 128'h02020202020202020202020202020202);
   cn = 2139; testcase_vvv(i8x16_add_sat_s,         128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2140; testcase_vvv(i8x16_add_sat_s,         128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2141; testcase_vvv(i8x16_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfefefefefefefefefefefefefefefefe);
   cn = 2142; testcase_vvv(i8x16_add_sat_s,         128'h3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f, 128'h40404040404040404040404040404040, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2143; testcase_vvv(i8x16_add_sat_s,         128'h40404040404040404040404040404040, 128'h40404040404040404040404040404040, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2144; testcase_vvv(i8x16_add_sat_s,         128'hc1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h81818181818181818181818181818181);
   cn = 2145; testcase_vvv(i8x16_add_sat_s,         128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h80808080808080808080808080808080);
   cn = 2146; testcase_vvv(i8x16_add_sat_s,         128'hbfbfbfbfbfbfbfbfbfbfbfbfbfbfbfbf, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h80808080808080808080808080808080);
   cn = 2147; testcase_vvv(i8x16_add_sat_s,         128'h7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d, 128'h01010101010101010101010101010101, 128'h7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e);
   cn = 2148; testcase_vvv(i8x16_add_sat_s,         128'h7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e, 128'h01010101010101010101010101010101, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2149; testcase_vvv(i8x16_add_sat_s,         128'h80808080808080808080808080808080, 128'h01010101010101010101010101010101, 128'h81818181818181818181818181818181);
   cn = 2150; testcase_vvv(i8x16_add_sat_s,         128'h82828282828282828282828282828282, 128'hffffffffffffffffffffffffffffffff, 128'h81818181818181818181818181818181);
   cn = 2151; testcase_vvv(i8x16_add_sat_s,         128'h81818181818181818181818181818181, 128'hffffffffffffffffffffffffffffffff, 128'h80808080808080808080808080808080);
   cn = 2152; testcase_vvv(i8x16_add_sat_s,         128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff, 128'h80808080808080808080808080808080);
   cn = 2153; testcase_vvv(i8x16_add_sat_s,         128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2154; testcase_vvv(i8x16_add_sat_s,         128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 2155; testcase_vvv(i8x16_add_sat_s,         128'h80808080808080808080808080808080, 128'h81818181818181818181818181818181, 128'h80808080808080808080808080808080);
   cn = 2156; testcase_vvv(i8x16_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2157; testcase_vvv(i8x16_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101, 128'h00000000000000000000000000000000);
   cn = 2158; testcase_vvv(i8x16_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfefefefefefefefefefefefefefefefe);
   cn = 2159; testcase_vvv(i8x16_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e);
   cn = 2160; testcase_vvv(i8x16_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 2161; testcase_vvv(i8x16_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfefefefefefefefefefefefefefefefe);
   cn = 2162; testcase_vvv(i8x16_add_sat_s,         128'h3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f, 128'h40404040404040404040404040404040, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2163; testcase_vvv(i8x16_add_sat_s,         128'h40404040404040404040404040404040, 128'h40404040404040404040404040404040, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2164; testcase_vvv(i8x16_add_sat_s,         128'hc1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h81818181818181818181818181818181);
   cn = 2165; testcase_vvv(i8x16_add_sat_s,         128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h80808080808080808080808080808080);
   cn = 2166; testcase_vvv(i8x16_add_sat_s,         128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hbfbfbfbfbfbfbfbfbfbfbfbfbfbfbfbf, 128'h80808080808080808080808080808080);
   cn = 2167; testcase_vvv(i8x16_add_sat_s,         128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2168; testcase_vvv(i8x16_add_sat_s,         128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h01010101010101010101010101010101, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2169; testcase_vvv(i8x16_add_sat_s,         128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff, 128'h80808080808080808080808080808080);
   cn = 2170; testcase_vvv(i8x16_add_sat_s,         128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 2171; testcase_vvv(i8x16_add_sat_s,         128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080);
   cn = 2172; testcase_vvv(i8x16_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101, 128'h00000000000000000000000000000000);
   cn = 2173; testcase_vvv(i8x16_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfefefefefefefefefefefefefefefefe);
   cn = 2174; testcase_vvv(i8x16_add_sat_s,         128'h000102030405060708090a0b0c0d0e0f, 128'h00fffefdfcfbfaf9f8f7f6f5f4f3f2f1, 128'h00000000000000000000000000000000);
   cn = 2175; testcase_vvv(i8x16_add_sat_s,         128'h000102030405060708090a0b0c0d0e0f, 128'h00020406080a0c0e10121416181a1c1e, 128'h000306090c0f1215181b1e2124272a2d);

   cn = 2048; testcase_vvv(i16x8_add_sat_s,         128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 2049; testcase_vvv(i16x8_add_sat_s,         128'h00000000000000000000000000000000, 128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001);
   cn = 2050; testcase_vvv(i16x8_add_sat_s,         128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001, 128'h00020002000200020002000200020002);
   cn = 2051; testcase_vvv(i16x8_add_sat_s,         128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2052; testcase_vvv(i16x8_add_sat_s,         128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2053; testcase_vvv(i16x8_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfffefffefffefffefffefffefffefffe);
   cn = 2054; testcase_vvv(i16x8_add_sat_s,         128'h3fff3fff3fff3fff3fff3fff3fff3fff, 128'h40004000400040004000400040004000, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2055; testcase_vvv(i16x8_add_sat_s,         128'h40004000400040004000400040004000, 128'h40004000400040004000400040004000, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2056; testcase_vvv(i16x8_add_sat_s,         128'hc001c001c001c001c001c001c001c001, 128'hc000c000c000c000c000c000c000c000, 128'h80018001800180018001800180018001);
   cn = 2057; testcase_vvv(i16x8_add_sat_s,         128'hc000c000c000c000c000c000c000c000, 128'hc000c000c000c000c000c000c000c000, 128'h80008000800080008000800080008000);
   cn = 2058; testcase_vvv(i16x8_add_sat_s,         128'hbfffbfffbfffbfffbfffbfffbfffbfff, 128'hc000c000c000c000c000c000c000c000, 128'h80008000800080008000800080008000);
   cn = 2059; testcase_vvv(i16x8_add_sat_s,         128'h7ffd7ffd7ffd7ffd7ffd7ffd7ffd7ffd, 128'h00010001000100010001000100010001, 128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe);
   cn = 2060; testcase_vvv(i16x8_add_sat_s,         128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe, 128'h00010001000100010001000100010001, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2061; testcase_vvv(i16x8_add_sat_s,         128'h80008000800080008000800080008000, 128'h00010001000100010001000100010001, 128'h80018001800180018001800180018001);
   cn = 2062; testcase_vvv(i16x8_add_sat_s,         128'h80028002800280028002800280028002, 128'hffffffffffffffffffffffffffffffff, 128'h80018001800180018001800180018001);
   cn = 2063; testcase_vvv(i16x8_add_sat_s,         128'h80018001800180018001800180018001, 128'hffffffffffffffffffffffffffffffff, 128'h80008000800080008000800080008000);
   cn = 2064; testcase_vvv(i16x8_add_sat_s,         128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff, 128'h80008000800080008000800080008000);
   cn = 2065; testcase_vvv(i16x8_add_sat_s,         128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2066; testcase_vvv(i16x8_add_sat_s,         128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 2067; testcase_vvv(i16x8_add_sat_s,         128'h80008000800080008000800080008000, 128'h80018001800180018001800180018001, 128'h80008000800080008000800080008000);
   cn = 2068; testcase_vvv(i16x8_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2069; testcase_vvv(i16x8_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'h00000000000000000000000000000000);
   cn = 2070; testcase_vvv(i16x8_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfffefffefffefffefffefffefffefffe);
   cn = 2071; testcase_vvv(i16x8_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe);
   cn = 2072; testcase_vvv(i16x8_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 2073; testcase_vvv(i16x8_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfffefffefffefffefffefffefffefffe);
   cn = 2074; testcase_vvv(i16x8_add_sat_s,         128'h3fff3fff3fff3fff3fff3fff3fff3fff, 128'h40004000400040004000400040004000, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2075; testcase_vvv(i16x8_add_sat_s,         128'h40004000400040004000400040004000, 128'h40004000400040004000400040004000, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2076; testcase_vvv(i16x8_add_sat_s,         128'hc001c001c001c001c001c001c001c001, 128'hc000c000c000c000c000c000c000c000, 128'h80018001800180018001800180018001);
   cn = 2077; testcase_vvv(i16x8_add_sat_s,         128'hc000c000c000c000c000c000c000c000, 128'hc000c000c000c000c000c000c000c000, 128'h80008000800080008000800080008000);
   cn = 2078; testcase_vvv(i16x8_add_sat_s,         128'hc000c000c000c000c000c000c000c000, 128'hbfffbfffbfffbfffbfffbfffbfffbfff, 128'h80008000800080008000800080008000);
   cn = 2079; testcase_vvv(i16x8_add_sat_s,         128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2080; testcase_vvv(i16x8_add_sat_s,         128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h00010001000100010001000100010001, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2081; testcase_vvv(i16x8_add_sat_s,         128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff, 128'h80008000800080008000800080008000);
   cn = 2082; testcase_vvv(i16x8_add_sat_s,         128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2083; testcase_vvv(i16x8_add_sat_s,         128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000);
   cn = 2084; testcase_vvv(i16x8_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'h00000000000000000000000000000000);
   cn = 2085; testcase_vvv(i16x8_add_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hfffefffefffefffefffefffefffefffe);
   cn = 2086; testcase_vvv(i16x8_add_sat_s,         128'h00000001000200030004000500060007, 128'h0000fffffffefffdfffcfffbfffafff9, 128'h00000000000000000000000000000000);
   cn = 2087; testcase_vvv(i16x8_add_sat_s,         128'h00000001000200030004000500060007, 128'h00000002000400060008000a000c000e, 128'h0000000300060009000c000f00120015);
   cn = 2088; testcase_vvv(i16x8_add_sat_s,         128'h30393039303930393039303930393039, 128'h7d7b7d7b7d7b7d7b7d7b7d7b7d7b7d7b, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2089; testcase_vvv(i16x8_add_sat_s,         128'h30393039303930393039303930393039, 128'hddd5ddd5ddd5ddd5ddd5ddd5ddd5ddd5, 128'h0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e);
   cn = 2090; testcase_vvv(i16x8_add_sat_s,         128'h12341234123412341234123412341234, 128'h56785678567856785678567856785678, 128'h68ac68ac68ac68ac68ac68ac68ac68ac);
   cn = 2091; testcase_vvv(i16x8_add_sat_s,         128'h90ab90ab90ab90ab90ab90ab90ab90ab, 128'hcdefcdefcdefcdefcdefcdefcdefcdef, 128'h80008000800080008000800080008000);

   $display(" Testing  add_sat_u");
   cn = 2176; testcase_vvv(i8x16_add_sat_u,         128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 2177; testcase_vvv(i8x16_add_sat_u,         128'h00000000000000000000000000000000, 128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101);
   cn = 2178; testcase_vvv(i8x16_add_sat_u,         128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101, 128'h02020202020202020202020202020202);
   cn = 2179; testcase_vvv(i8x16_add_sat_u,         128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2180; testcase_vvv(i8x16_add_sat_u,         128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2181; testcase_vvv(i8x16_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2182; testcase_vvv(i8x16_add_sat_u,         128'h3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f, 128'h40404040404040404040404040404040, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2183; testcase_vvv(i8x16_add_sat_u,         128'h40404040404040404040404040404040, 128'h40404040404040404040404040404040, 128'h80808080808080808080808080808080);
   cn = 2184; testcase_vvv(i8x16_add_sat_u,         128'hc1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hffffffffffffffffffffffffffffffff);
   cn = 2185; testcase_vvv(i8x16_add_sat_u,         128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hffffffffffffffffffffffffffffffff);
   cn = 2186; testcase_vvv(i8x16_add_sat_u,         128'hbfbfbfbfbfbfbfbfbfbfbfbfbfbfbfbf, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hffffffffffffffffffffffffffffffff);
   cn = 2187; testcase_vvv(i8x16_add_sat_u,         128'h7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d, 128'h01010101010101010101010101010101, 128'h7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e);
   cn = 2188; testcase_vvv(i8x16_add_sat_u,         128'h7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e, 128'h01010101010101010101010101010101, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2189; testcase_vvv(i8x16_add_sat_u,         128'h80808080808080808080808080808080, 128'h01010101010101010101010101010101, 128'h81818181818181818181818181818181);
   cn = 2190; testcase_vvv(i8x16_add_sat_u,         128'h82828282828282828282828282828282, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2191; testcase_vvv(i8x16_add_sat_u,         128'h81818181818181818181818181818181, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2192; testcase_vvv(i8x16_add_sat_u,         128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2193; testcase_vvv(i8x16_add_sat_u,         128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'hfefefefefefefefefefefefefefefefe);
   cn = 2194; testcase_vvv(i8x16_add_sat_u,         128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 2195; testcase_vvv(i8x16_add_sat_u,         128'h80808080808080808080808080808080, 128'h81818181818181818181818181818181, 128'hffffffffffffffffffffffffffffffff);
   cn = 2196; testcase_vvv(i8x16_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2197; testcase_vvv(i8x16_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff);
   cn = 2198; testcase_vvv(i8x16_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2199; testcase_vvv(i8x16_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'hffffffffffffffffffffffffffffffff);
   cn = 2200; testcase_vvv(i8x16_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 2201; testcase_vvv(i8x16_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2202; testcase_vvv(i8x16_add_sat_u,         128'h3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f, 128'h40404040404040404040404040404040, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2203; testcase_vvv(i8x16_add_sat_u,         128'h40404040404040404040404040404040, 128'h40404040404040404040404040404040, 128'h80808080808080808080808080808080);
   cn = 2204; testcase_vvv(i8x16_add_sat_u,         128'hc1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hffffffffffffffffffffffffffffffff);
   cn = 2205; testcase_vvv(i8x16_add_sat_u,         128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hffffffffffffffffffffffffffffffff);
   cn = 2206; testcase_vvv(i8x16_add_sat_u,         128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hbfbfbfbfbfbfbfbfbfbfbfbfbfbfbfbf, 128'hffffffffffffffffffffffffffffffff);
   cn = 2207; testcase_vvv(i8x16_add_sat_u,         128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'hfefefefefefefefefefefefefefefefe);
   cn = 2208; testcase_vvv(i8x16_add_sat_u,         128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h01010101010101010101010101010101, 128'h80808080808080808080808080808080);
   cn = 2209; testcase_vvv(i8x16_add_sat_u,         128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2210; testcase_vvv(i8x16_add_sat_u,         128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 2211; testcase_vvv(i8x16_add_sat_u,         128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff);
   cn = 2212; testcase_vvv(i8x16_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff);
   cn = 2213; testcase_vvv(i8x16_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2214; testcase_vvv(i8x16_add_sat_u,         128'h000102030405060708090a0b0c0d0e0f, 128'h00fffefdfcfbfaf9f8f7f6f5f4f3f2f1, 128'h00ffffffffffffffffffffffffffffff);
   cn = 2215; testcase_vvv(i8x16_add_sat_u,         128'h000102030405060708090a0b0c0d0e0f, 128'h00020406080a0c0e10121416181a1c1e, 128'h000306090c0f1215181b1e2124272a2d);

   cn = 2092; testcase_vvv(i16x8_add_sat_u,         128'h00000000_00000000_00000000_00000000, 128'h00000000_00000000_00000000_00000000, 128'h00000000_00000000_00000000_00000000);
   cn = 2093; testcase_vvv(i16x8_add_sat_u,         128'h00000000000000000000000000000000, 128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001);
   cn = 2094; testcase_vvv(i16x8_add_sat_u,         128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001, 128'h00020002000200020002000200020002);
   cn = 2095; testcase_vvv(i16x8_add_sat_u,         128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2096; testcase_vvv(i16x8_add_sat_u,         128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2097; testcase_vvv(i16x8_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2098; testcase_vvv(i16x8_add_sat_u,         128'h3fff3fff3fff3fff3fff3fff3fff3fff, 128'h40004000400040004000400040004000, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2099; testcase_vvv(i16x8_add_sat_u,         128'h40004000400040004000400040004000, 128'h40004000400040004000400040004000, 128'h80008000800080008000800080008000);
   cn = 2100; testcase_vvv(i16x8_add_sat_u,         128'hc001c001c001c001c001c001c001c001, 128'hc000c000c000c000c000c000c000c000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2101; testcase_vvv(i16x8_add_sat_u,         128'hc000c000c000c000c000c000c000c000, 128'hc000c000c000c000c000c000c000c000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2102; testcase_vvv(i16x8_add_sat_u,         128'hbfffbfffbfffbfffbfffbfffbfffbfff, 128'hc000c000c000c000c000c000c000c000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2103; testcase_vvv(i16x8_add_sat_u,         128'h7ffd7ffd7ffd7ffd7ffd7ffd7ffd7ffd, 128'h00010001000100010001000100010001, 128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe);
   cn = 2104; testcase_vvv(i16x8_add_sat_u,         128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe, 128'h00010001000100010001000100010001, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2105; testcase_vvv(i16x8_add_sat_u,         128'h80008000800080008000800080008000, 128'h00010001000100010001000100010001, 128'h80018001800180018001800180018001);
   cn = 2106; testcase_vvv(i16x8_add_sat_u,         128'h80028002800280028002800280028002, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2107; testcase_vvv(i16x8_add_sat_u,         128'h80018001800180018001800180018001, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2108; testcase_vvv(i16x8_add_sat_u,         128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2109; testcase_vvv(i16x8_add_sat_u,         128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'hfffefffefffefffefffefffefffefffe);
   cn = 2110; testcase_vvv(i16x8_add_sat_u,         128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2111; testcase_vvv(i16x8_add_sat_u,         128'h80008000800080008000800080008000, 128'h80018001800180018001800180018001, 128'hffffffffffffffffffffffffffffffff);
   cn = 2112; testcase_vvv(i16x8_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2113; testcase_vvv(i16x8_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff);
   cn = 2114; testcase_vvv(i16x8_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2115; testcase_vvv(i16x8_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2116; testcase_vvv(i16x8_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2117; testcase_vvv(i16x8_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2118; testcase_vvv(i16x8_add_sat_u,         128'h3fff3fff3fff3fff3fff3fff3fff3fff, 128'h40004000400040004000400040004000, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2119; testcase_vvv(i16x8_add_sat_u,         128'h40004000400040004000400040004000, 128'h40004000400040004000400040004000, 128'h80008000800080008000800080008000);
   cn = 2120; testcase_vvv(i16x8_add_sat_u,         128'hc001c001c001c001c001c001c001c001, 128'hc000c000c000c000c000c000c000c000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2121; testcase_vvv(i16x8_add_sat_u,         128'hc000c000c000c000c000c000c000c000, 128'hc000c000c000c000c000c000c000c000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2122; testcase_vvv(i16x8_add_sat_u,         128'hc000c000c000c000c000c000c000c000, 128'hbfffbfffbfffbfffbfffbfffbfffbfff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2123; testcase_vvv(i16x8_add_sat_u,         128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'hfffefffefffefffefffefffefffefffe);
   cn = 2124; testcase_vvv(i16x8_add_sat_u,         128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h00010001000100010001000100010001, 128'h80008000800080008000800080008000);
   cn = 2125; testcase_vvv(i16x8_add_sat_u,         128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2126; testcase_vvv(i16x8_add_sat_u,         128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2127; testcase_vvv(i16x8_add_sat_u,         128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2128; testcase_vvv(i16x8_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff);
   cn = 2129; testcase_vvv(i16x8_add_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2130; testcase_vvv(i16x8_add_sat_u,         128'h00000001000200030004000500060007, 128'h0000fffffffefffdfffcfffbfffafff9, 128'h0000ffffffffffffffffffffffffffff);
   cn = 2131; testcase_vvv(i16x8_add_sat_u,         128'h00000001000200030004000500060007, 128'h00000002000400060008000a000c000e, 128'h0000000300060009000c000f00120015);
   cn = 2132; testcase_vvv(i16x8_add_sat_u,         128'h30393039303930393039303930393039, 128'hddd5ddd5ddd5ddd5ddd5ddd5ddd5ddd5, 128'hffffffffffffffffffffffffffffffff);
   cn = 2133; testcase_vvv(i16x8_add_sat_u,         128'h30393039303930393039303930393039, 128'hcfc7cfc7cfc7cfc7cfc7cfc7cfc7cfc7, 128'hffffffffffffffffffffffffffffffff);
   cn = 2134; testcase_vvv(i16x8_add_sat_u,         128'h12341234123412341234123412341234, 128'h56785678567856785678567856785678, 128'h68ac68ac68ac68ac68ac68ac68ac68ac);
   cn = 2135; testcase_vvv(i16x8_add_sat_u,         128'h90ab90ab90ab90ab90ab90ab90ab90ab, 128'hcdefcdefcdefcdefcdefcdefcdefcdef, 128'hffffffffffffffffffffffffffffffff);

   $display(" Testing  sub_sat_s");
   cn = 2304; testcase_vvv(i8x16_sub_sat_s,         128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 2305; testcase_vvv(i8x16_sub_sat_s,         128'h00000000000000000000000000000000, 128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff);
   cn = 2306; testcase_vvv(i8x16_sub_sat_s,         128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101, 128'h00000000000000000000000000000000);
   cn = 2307; testcase_vvv(i8x16_sub_sat_s,         128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101);
   cn = 2308; testcase_vvv(i8x16_sub_sat_s,         128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff, 128'h02020202020202020202020202020202);
   cn = 2309; testcase_vvv(i8x16_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2310; testcase_vvv(i8x16_sub_sat_s,         128'h3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f, 128'h40404040404040404040404040404040, 128'hffffffffffffffffffffffffffffffff);
   cn = 2311; testcase_vvv(i8x16_sub_sat_s,         128'h40404040404040404040404040404040, 128'h40404040404040404040404040404040, 128'h00000000000000000000000000000000);
   cn = 2312; testcase_vvv(i8x16_sub_sat_s,         128'hc1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h01010101010101010101010101010101);
   cn = 2313; testcase_vvv(i8x16_sub_sat_s,         128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h00000000000000000000000000000000);
   cn = 2314; testcase_vvv(i8x16_sub_sat_s,         128'hbfbfbfbfbfbfbfbfbfbfbfbfbfbfbfbf, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hffffffffffffffffffffffffffffffff);
   cn = 2315; testcase_vvv(i8x16_sub_sat_s,         128'h7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d, 128'h01010101010101010101010101010101, 128'h7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c);
   cn = 2316; testcase_vvv(i8x16_sub_sat_s,         128'h7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e, 128'h01010101010101010101010101010101, 128'h7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d);
   cn = 2317; testcase_vvv(i8x16_sub_sat_s,         128'h80808080808080808080808080808080, 128'h01010101010101010101010101010101, 128'h80808080808080808080808080808080);
   cn = 2318; testcase_vvv(i8x16_sub_sat_s,         128'h82828282828282828282828282828282, 128'hffffffffffffffffffffffffffffffff, 128'h83838383838383838383838383838383);
   cn = 2319; testcase_vvv(i8x16_sub_sat_s,         128'h81818181818181818181818181818181, 128'hffffffffffffffffffffffffffffffff, 128'h82828282828282828282828282828282);
   cn = 2320; testcase_vvv(i8x16_sub_sat_s,         128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff, 128'h81818181818181818181818181818181);
   cn = 2321; testcase_vvv(i8x16_sub_sat_s,         128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h00000000000000000000000000000000);
   cn = 2322; testcase_vvv(i8x16_sub_sat_s,         128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 2323; testcase_vvv(i8x16_sub_sat_s,         128'h80808080808080808080808080808080, 128'h81818181818181818181818181818181, 128'hffffffffffffffffffffffffffffffff);
   cn = 2324; testcase_vvv(i8x16_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2325; testcase_vvv(i8x16_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101, 128'hfefefefefefefefefefefefefefefefe);
   cn = 2326; testcase_vvv(i8x16_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2327; testcase_vvv(i8x16_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h80808080808080808080808080808080);
   cn = 2328; testcase_vvv(i8x16_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h80808080808080808080808080808080, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2329; testcase_vvv(i8x16_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2330; testcase_vvv(i8x16_sub_sat_s,         128'h3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f, 128'h40404040404040404040404040404040, 128'hffffffffffffffffffffffffffffffff);
   cn = 2331; testcase_vvv(i8x16_sub_sat_s,         128'h40404040404040404040404040404040, 128'h40404040404040404040404040404040, 128'h00000000000000000000000000000000);
   cn = 2332; testcase_vvv(i8x16_sub_sat_s,         128'hc1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h01010101010101010101010101010101);
   cn = 2333; testcase_vvv(i8x16_sub_sat_s,         128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h00000000000000000000000000000000);
   cn = 2334; testcase_vvv(i8x16_sub_sat_s,         128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hbfbfbfbfbfbfbfbfbfbfbfbfbfbfbfbf, 128'h01010101010101010101010101010101);
   cn = 2335; testcase_vvv(i8x16_sub_sat_s,         128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h00000000000000000000000000000000);
   cn = 2336; testcase_vvv(i8x16_sub_sat_s,         128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h01010101010101010101010101010101, 128'h7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e);
   cn = 2337; testcase_vvv(i8x16_sub_sat_s,         128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff, 128'h81818181818181818181818181818181);
   cn = 2338; testcase_vvv(i8x16_sub_sat_s,         128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h80808080808080808080808080808080, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2339; testcase_vvv(i8x16_sub_sat_s,         128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 2340; testcase_vvv(i8x16_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101, 128'hfefefefefefefefefefefefefefefefe);
   cn = 2341; testcase_vvv(i8x16_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2342; testcase_vvv(i8x16_sub_sat_s,         128'h000102030405060708090a0b0c0d0e0f, 128'h00fffefdfcfbfaf9f8f7f6f5f4f3f2f1, 128'h00020406080a0c0e10121416181a1c1e);
   cn = 2343; testcase_vvv(i8x16_sub_sat_s,         128'h000102030405060708090a0b0c0d0e0f, 128'h00020406080a0c0e10121416181a1c1e, 128'h00fffefdfcfbfaf9f8f7f6f5f4f3f2f1);

   cn = 2216; testcase_vvv(i16x8_sub_sat_s,         128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 2217; testcase_vvv(i16x8_sub_sat_s,         128'h00000000000000000000000000000000, 128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff);
   cn = 2218; testcase_vvv(i16x8_sub_sat_s,         128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001, 128'h00000000000000000000000000000000);
   cn = 2219; testcase_vvv(i16x8_sub_sat_s,         128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001);
   cn = 2220; testcase_vvv(i16x8_sub_sat_s,         128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff, 128'h00020002000200020002000200020002);
   cn = 2221; testcase_vvv(i16x8_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2222; testcase_vvv(i16x8_sub_sat_s,         128'h3fff3fff3fff3fff3fff3fff3fff3fff, 128'h40004000400040004000400040004000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2223; testcase_vvv(i16x8_sub_sat_s,         128'h40004000400040004000400040004000, 128'h40004000400040004000400040004000, 128'h00000000000000000000000000000000);
   cn = 2224; testcase_vvv(i16x8_sub_sat_s,         128'hc001c001c001c001c001c001c001c001, 128'hc000c000c000c000c000c000c000c000, 128'h00010001000100010001000100010001);
   cn = 2225; testcase_vvv(i16x8_sub_sat_s,         128'hc000c000c000c000c000c000c000c000, 128'hc000c000c000c000c000c000c000c000, 128'h00000000000000000000000000000000);
   cn = 2226; testcase_vvv(i16x8_sub_sat_s,         128'hbfffbfffbfffbfffbfffbfffbfffbfff, 128'hc000c000c000c000c000c000c000c000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2227; testcase_vvv(i16x8_sub_sat_s,         128'h7ffd7ffd7ffd7ffd7ffd7ffd7ffd7ffd, 128'h00010001000100010001000100010001, 128'h7ffc7ffc7ffc7ffc7ffc7ffc7ffc7ffc);
   cn = 2228; testcase_vvv(i16x8_sub_sat_s,         128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe, 128'h00010001000100010001000100010001, 128'h7ffd7ffd7ffd7ffd7ffd7ffd7ffd7ffd);
   cn = 2229; testcase_vvv(i16x8_sub_sat_s,         128'h80008000800080008000800080008000, 128'h00010001000100010001000100010001, 128'h80008000800080008000800080008000);
   cn = 2230; testcase_vvv(i16x8_sub_sat_s,         128'h80028002800280028002800280028002, 128'hffffffffffffffffffffffffffffffff, 128'h80038003800380038003800380038003);
   cn = 2231; testcase_vvv(i16x8_sub_sat_s,         128'h80018001800180018001800180018001, 128'hffffffffffffffffffffffffffffffff, 128'h80028002800280028002800280028002);
   cn = 2232; testcase_vvv(i16x8_sub_sat_s,         128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff, 128'h80018001800180018001800180018001);
   cn = 2233; testcase_vvv(i16x8_sub_sat_s,         128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h00000000000000000000000000000000);
   cn = 2234; testcase_vvv(i16x8_sub_sat_s,         128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h00000000000000000000000000000000);
   cn = 2235; testcase_vvv(i16x8_sub_sat_s,         128'h80008000800080008000800080008000, 128'h80018001800180018001800180018001, 128'hffffffffffffffffffffffffffffffff);
   cn = 2236; testcase_vvv(i16x8_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2237; testcase_vvv(i16x8_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'hfffefffefffefffefffefffefffefffe);
   cn = 2238; testcase_vvv(i16x8_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2239; testcase_vvv(i16x8_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h80008000800080008000800080008000);
   cn = 2240; testcase_vvv(i16x8_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h80008000800080008000800080008000, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2241; testcase_vvv(i16x8_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2242; testcase_vvv(i16x8_sub_sat_s,         128'h3fff3fff3fff3fff3fff3fff3fff3fff, 128'h40004000400040004000400040004000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2243; testcase_vvv(i16x8_sub_sat_s,         128'h40004000400040004000400040004000, 128'h40004000400040004000400040004000, 128'h00000000000000000000000000000000);
   cn = 2244; testcase_vvv(i16x8_sub_sat_s,         128'hc001c001c001c001c001c001c001c001, 128'hc000c000c000c000c000c000c000c000, 128'h00010001000100010001000100010001);
   cn = 2245; testcase_vvv(i16x8_sub_sat_s,         128'hc000c000c000c000c000c000c000c000, 128'hc000c000c000c000c000c000c000c000, 128'h00000000000000000000000000000000);
   cn = 2246; testcase_vvv(i16x8_sub_sat_s,         128'hc000c000c000c000c000c000c000c000, 128'hbfffbfffbfffbfffbfffbfffbfffbfff, 128'h00010001000100010001000100010001);
   cn = 2247; testcase_vvv(i16x8_sub_sat_s,         128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h00000000000000000000000000000000);
   cn = 2248; testcase_vvv(i16x8_sub_sat_s,         128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h00010001000100010001000100010001, 128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe);
   cn = 2249; testcase_vvv(i16x8_sub_sat_s,         128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff, 128'h80018001800180018001800180018001);
   cn = 2250; testcase_vvv(i16x8_sub_sat_s,         128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h80008000800080008000800080008000, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2251; testcase_vvv(i16x8_sub_sat_s,         128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h00000000000000000000000000000000);
   cn = 2252; testcase_vvv(i16x8_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'hfffefffefffefffefffefffefffefffe);
   cn = 2253; testcase_vvv(i16x8_sub_sat_s,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2254; testcase_vvv(i16x8_sub_sat_s,         128'h00000001000200030004000500060007, 128'h0000fffffffefffdfffcfffbfffafff9, 128'h00000002000400060008000a000c000e);
   cn = 2255; testcase_vvv(i16x8_sub_sat_s,         128'h00000001000200030004000500060007, 128'h00000002000400060008000a000c000e, 128'h0000fffffffefffdfffcfffbfffafff9);
   cn = 2256; testcase_vvv(i16x8_sub_sat_s,         128'h30393039303930393039303930393039, 128'hddd5ddd5ddd5ddd5ddd5ddd5ddd5ddd5, 128'h52645264526452645264526452645264);
   cn = 2257; testcase_vvv(i16x8_sub_sat_s,         128'h30393039303930393039303930393039, 128'hcfc7cfc7cfc7cfc7cfc7cfc7cfc7cfc7, 128'h60726072607260726072607260726072);
   cn = 2258; testcase_vvv(i16x8_sub_sat_s,         128'h12341234123412341234123412341234, 128'h56785678567856785678567856785678, 128'hbbbcbbbcbbbcbbbcbbbcbbbcbbbcbbbc);
   cn = 2259; testcase_vvv(i16x8_sub_sat_s,         128'h90ab90ab90ab90ab90ab90ab90ab90ab, 128'hedccedccedccedccedccedccedccedcc, 128'ha2dfa2dfa2dfa2dfa2dfa2dfa2dfa2df);

   $display(" Testing  sub_sat_u");
   cn = 2344; testcase_vvv(i8x16_sub_sat_u,         128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 2345; testcase_vvv(i8x16_sub_sat_u,         128'h00000000000000000000000000000000, 128'h01010101010101010101010101010101, 128'h00000000000000000000000000000000);
   cn = 2346; testcase_vvv(i8x16_sub_sat_u,         128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101, 128'h00000000000000000000000000000000);
   cn = 2347; testcase_vvv(i8x16_sub_sat_u,         128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2348; testcase_vvv(i8x16_sub_sat_u,         128'h01010101010101010101010101010101, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2349; testcase_vvv(i8x16_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2350; testcase_vvv(i8x16_sub_sat_u,         128'h3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f, 128'h40404040404040404040404040404040, 128'h00000000000000000000000000000000);
   cn = 2351; testcase_vvv(i8x16_sub_sat_u,         128'h40404040404040404040404040404040, 128'h40404040404040404040404040404040, 128'h00000000000000000000000000000000);
   cn = 2352; testcase_vvv(i8x16_sub_sat_u,         128'hc1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h01010101010101010101010101010101);
   cn = 2353; testcase_vvv(i8x16_sub_sat_u,         128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h00000000000000000000000000000000);
   cn = 2354; testcase_vvv(i8x16_sub_sat_u,         128'hbfbfbfbfbfbfbfbfbfbfbfbfbfbfbfbf, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h00000000000000000000000000000000);
   cn = 2355; testcase_vvv(i8x16_sub_sat_u,         128'h7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d, 128'h01010101010101010101010101010101, 128'h7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c);
   cn = 2356; testcase_vvv(i8x16_sub_sat_u,         128'h7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e, 128'h01010101010101010101010101010101, 128'h7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d7d);
   cn = 2357; testcase_vvv(i8x16_sub_sat_u,         128'h80808080808080808080808080808080, 128'h01010101010101010101010101010101, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2358; testcase_vvv(i8x16_sub_sat_u,         128'h82828282828282828282828282828282, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2359; testcase_vvv(i8x16_sub_sat_u,         128'h81818181818181818181818181818181, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2360; testcase_vvv(i8x16_sub_sat_u,         128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2361; testcase_vvv(i8x16_sub_sat_u,         128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h00000000000000000000000000000000);
   cn = 2362; testcase_vvv(i8x16_sub_sat_u,         128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 2363; testcase_vvv(i8x16_sub_sat_u,         128'h80808080808080808080808080808080, 128'h81818181818181818181818181818181, 128'h00000000000000000000000000000000);
   cn = 2364; testcase_vvv(i8x16_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2365; testcase_vvv(i8x16_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101, 128'hfefefefefefefefefefefefefefefefe);
   cn = 2366; testcase_vvv(i8x16_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2367; testcase_vvv(i8x16_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h80808080808080808080808080808080);
   cn = 2368; testcase_vvv(i8x16_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h80808080808080808080808080808080, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2369; testcase_vvv(i8x16_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2370; testcase_vvv(i8x16_sub_sat_u,         128'h3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f, 128'h40404040404040404040404040404040, 128'h00000000000000000000000000000000);
   cn = 2371; testcase_vvv(i8x16_sub_sat_u,         128'h40404040404040404040404040404040, 128'h40404040404040404040404040404040, 128'h00000000000000000000000000000000);
   cn = 2372; testcase_vvv(i8x16_sub_sat_u,         128'hc1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h01010101010101010101010101010101);
   cn = 2373; testcase_vvv(i8x16_sub_sat_u,         128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'h00000000000000000000000000000000);
   cn = 2374; testcase_vvv(i8x16_sub_sat_u,         128'hc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0, 128'hbfbfbfbfbfbfbfbfbfbfbfbfbfbfbfbf, 128'h01010101010101010101010101010101);
   cn = 2375; testcase_vvv(i8x16_sub_sat_u,         128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h00000000000000000000000000000000);
   cn = 2376; testcase_vvv(i8x16_sub_sat_u,         128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h01010101010101010101010101010101, 128'h7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e);
   cn = 2377; testcase_vvv(i8x16_sub_sat_u,         128'h80808080808080808080808080808080, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2378; testcase_vvv(i8x16_sub_sat_u,         128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 2379; testcase_vvv(i8x16_sub_sat_u,         128'h80808080808080808080808080808080, 128'h80808080808080808080808080808080, 128'h00000000000000000000000000000000);
   cn = 2380; testcase_vvv(i8x16_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h01010101010101010101010101010101, 128'hfefefefefefefefefefefefefefefefe);
   cn = 2381; testcase_vvv(i8x16_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2382; testcase_vvv(i8x16_sub_sat_u,         128'h000102030405060708090a0b0c0d0e0f, 128'h00fffefdfcfbfaf9f8f7f6f5f4f3f2f1, 128'h00000000000000000000000000000000);
   cn = 2383; testcase_vvv(i8x16_sub_sat_u,         128'h000102030405060708090a0b0c0d0e0f, 128'h00020406080a0c0e10121416181a1c1e, 128'h00000000000000000000000000000000);

   cn = 2260; testcase_vvv(i16x8_sub_sat_u,         128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 2261; testcase_vvv(i16x8_sub_sat_u,         128'h00000000000000000000000000000000, 128'h00010001000100010001000100010001, 128'h00000000000000000000000000000000);
   cn = 2262; testcase_vvv(i16x8_sub_sat_u,         128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001, 128'h00000000000000000000000000000000);
   cn = 2263; testcase_vvv(i16x8_sub_sat_u,         128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2264; testcase_vvv(i16x8_sub_sat_u,         128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2265; testcase_vvv(i16x8_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2266; testcase_vvv(i16x8_sub_sat_u,         128'h3fff3fff3fff3fff3fff3fff3fff3fff, 128'h40004000400040004000400040004000, 128'h00000000000000000000000000000000);
   cn = 2267; testcase_vvv(i16x8_sub_sat_u,         128'h40004000400040004000400040004000, 128'h40004000400040004000400040004000, 128'h00000000000000000000000000000000);
   cn = 2268; testcase_vvv(i16x8_sub_sat_u,         128'hc001c001c001c001c001c001c001c001, 128'hc000c000c000c000c000c000c000c000, 128'h00010001000100010001000100010001);
   cn = 2269; testcase_vvv(i16x8_sub_sat_u,         128'hc000c000c000c000c000c000c000c000, 128'hc000c000c000c000c000c000c000c000, 128'h00000000000000000000000000000000);
   cn = 2270; testcase_vvv(i16x8_sub_sat_u,         128'hbfffbfffbfffbfffbfffbfffbfffbfff, 128'hc000c000c000c000c000c000c000c000, 128'h00000000000000000000000000000000);
   cn = 2271; testcase_vvv(i16x8_sub_sat_u,         128'h7ffd7ffd7ffd7ffd7ffd7ffd7ffd7ffd, 128'h00010001000100010001000100010001, 128'h7ffc7ffc7ffc7ffc7ffc7ffc7ffc7ffc);
   cn = 2272; testcase_vvv(i16x8_sub_sat_u,         128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe, 128'h00010001000100010001000100010001, 128'h7ffd7ffd7ffd7ffd7ffd7ffd7ffd7ffd);
   cn = 2273; testcase_vvv(i16x8_sub_sat_u,         128'h80008000800080008000800080008000, 128'h00010001000100010001000100010001, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2274; testcase_vvv(i16x8_sub_sat_u,         128'h80028002800280028002800280028002, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2275; testcase_vvv(i16x8_sub_sat_u,         128'h80018001800180018001800180018001, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2276; testcase_vvv(i16x8_sub_sat_u,         128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2277; testcase_vvv(i16x8_sub_sat_u,         128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h00000000000000000000000000000000);
   cn = 2278; testcase_vvv(i16x8_sub_sat_u,         128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h00000000000000000000000000000000);
   cn = 2279; testcase_vvv(i16x8_sub_sat_u,         128'h80008000800080008000800080008000, 128'h80018001800180018001800180018001, 128'h00000000000000000000000000000000);
   cn = 2280; testcase_vvv(i16x8_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff);
   cn = 2281; testcase_vvv(i16x8_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'hfffefffefffefffefffefffefffefffe);
   cn = 2282; testcase_vvv(i16x8_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2283; testcase_vvv(i16x8_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h80008000800080008000800080008000);
   cn = 2284; testcase_vvv(i16x8_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h80008000800080008000800080008000, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2285; testcase_vvv(i16x8_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2286; testcase_vvv(i16x8_sub_sat_u,         128'h3fff3fff3fff3fff3fff3fff3fff3fff, 128'h40004000400040004000400040004000, 128'h00000000000000000000000000000000);
   cn = 2287; testcase_vvv(i16x8_sub_sat_u,         128'h40004000400040004000400040004000, 128'h40004000400040004000400040004000, 128'h00000000000000000000000000000000);
   cn = 2288; testcase_vvv(i16x8_sub_sat_u,         128'hc001c001c001c001c001c001c001c001, 128'hc000c000c000c000c000c000c000c000, 128'h00010001000100010001000100010001);
   cn = 2289; testcase_vvv(i16x8_sub_sat_u,         128'hc000c000c000c000c000c000c000c000, 128'hc000c000c000c000c000c000c000c000, 128'h00000000000000000000000000000000);
   cn = 2290; testcase_vvv(i16x8_sub_sat_u,         128'hc000c000c000c000c000c000c000c000, 128'hbfffbfffbfffbfffbfffbfffbfffbfff, 128'h00010001000100010001000100010001);
   cn = 2291; testcase_vvv(i16x8_sub_sat_u,         128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h00000000000000000000000000000000);
   cn = 2292; testcase_vvv(i16x8_sub_sat_u,         128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h00010001000100010001000100010001, 128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe);
   cn = 2293; testcase_vvv(i16x8_sub_sat_u,         128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2294; testcase_vvv(i16x8_sub_sat_u,         128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h80008000800080008000800080008000, 128'h00000000000000000000000000000000);
   cn = 2295; testcase_vvv(i16x8_sub_sat_u,         128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h00000000000000000000000000000000);
   cn = 2296; testcase_vvv(i16x8_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'hfffefffefffefffefffefffefffefffe);
   cn = 2297; testcase_vvv(i16x8_sub_sat_u,         128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2298; testcase_vvv(i16x8_sub_sat_u,         128'h00000001000200030004000500060007, 128'h0000fffffffefffdfffcfffbfffafff9, 128'h00000000000000000000000000000000);
   cn = 2299; testcase_vvv(i16x8_sub_sat_u,         128'h00000001000200030004000500060007, 128'h00000002000400060008000a000c000e, 128'h00000000000000000000000000000000);
   cn = 2300; testcase_vvv(i16x8_sub_sat_u,         128'h30393039303930393039303930393039, 128'hddd5ddd5ddd5ddd5ddd5ddd5ddd5ddd5, 128'h00000000000000000000000000000000);
   cn = 2301; testcase_vvv(i16x8_sub_sat_u,         128'hddd5ddd5ddd5ddd5ddd5ddd5ddd5ddd5, 128'hcfc7cfc7cfc7cfc7cfc7cfc7cfc7cfc7, 128'h0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e);
   cn = 2302; testcase_vvv(i16x8_sub_sat_u,         128'h12341234123412341234123412341234, 128'ha988a988a988a988a988a988a988a988, 128'h00000000000000000000000000000000);
   cn = 2303; testcase_vvv(i16x8_sub_sat_u,         128'hcdefcdefcdefcdefcdefcdefcdefcdef, 128'h90ab90ab90ab90ab90ab90ab90ab90ab, 128'h3d443d443d443d443d443d443d443d44);
   `endif //ENABLE_NEGABSSAT

   $display(" Testing  popcount");
   cn = 2384; testcase_vv(i8x16_popcnt,             128'h01010101010101010101010101010101, 128'h01010101010101010101010101010101);
   cn = 2385; testcase_vv(i8x16_popcnt,             128'hffffffffffffffffffffffffffffffff, 128'h08080808080808080808080808080808);
   cn = 2386; testcase_vv(i8x16_popcnt,             128'h80808080808080808080808080808080, 128'h01010101010101010101010101010101);
   cn = 2387; testcase_vv(i8x16_popcnt,             128'h7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b, 128'h06060606060606060606060606060606);
   cn = 2388; testcase_vv(i8x16_popcnt,             128'h85858585858585858585858585858585, 128'h03030303030303030303030303030303);

   $display(" Testing  shuffle");
   cn = 2389; testcase_vvvv(i8x16_shuffle,          128'h0f0e0d0c0b0a09080706050403020100, 128'hffeeddccbbaa99887766554433221100, 128'h000102030405060708090a0b0c0d0e0f, 128'h00112233445566778899aabbccddeeff);
   cn = 2390; testcase_vvvv(i8x16_shuffle,          128'h0f0e0d0c0b0a09080706050403020100, 128'hffeeddccbbaa99887766554433221100, 128'h101112131415161718191a1b1c1d1e1f, 128'h000102030405060708090a0b0c0d0e0f);
   cn = 2391; testcase_vvvv(i8x16_shuffle,          128'h0f0e0d0c0b0a09080706050403020100, 128'hffeeddccbbaa99887766554433221100, 128'h10_01_12_03_14_05_16_07_18_09_1a_0b_1c_0d_1e_01, 128'h00_11_02_33_04_55_06_77_08_99_0a_bb_0c_dd_0e_11);
   cn = 2392; testcase_vvvv(i8x16_shuffle,          128'h0f0e0d0c0b0a090807060504030201ff, 128'hffeeddccbbaa99887766554433221100, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 2393; testcase_vvvv(i8x16_shuffle,          128'h0f0e0d0c0b0a090807060504030201ff, 128'hffeeddccbbaa99887766554433221100, 128'h10101010101010101010101010101010, 128'hffffffffffffffffffffffffffffffff);

   $display(" Testing  splat");
   cn = 2394; testcase_sv(v128_load8_splat,         32'h00000000, 128'h00000000000000000000000000000000);
   cn = 2395; testcase_sv(v128_load8_splat,         32'h00000001, 128'h01010101010101010101010101010101);
   cn = 2396; testcase_sv(v128_load8_splat,         32'h00000002, 128'h02020202020202020202020202020202);
   cn = 2397; testcase_sv(v128_load8_splat,         32'h00000003, 128'h03030303030303030303030303030303);
   cn = 2398; testcase_sv(v128_load8_splat,         32'h0000ffff, 128'hffffffffffffffffffffffffffffffff);

   cn = 2399; testcase_sv(v128_load16_splat,        32'h00000004, 128'h00040004000400040004000400040004);
   cn = 2400; testcase_sv(v128_load16_splat,        32'h00000005, 128'h00050005000500050005000500050005);
   cn = 2401; testcase_sv(v128_load16_splat,        32'h00001234, 128'h12341234123412341234123412341234);
   cn = 2402; testcase_sv(v128_load16_splat,        32'h0000f0f0, 128'hf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0);
   cn = 2403; testcase_sv(v128_load16_splat,        32'h0000fffe, 128'hfffefffefffefffefffefffefffefffe);

   cn = 2404; testcase_sv(v128_load32_splat,        32'h00000008, 128'h00000008000000080000000800000008);
   cn = 2405; testcase_sv(v128_load32_splat,        32'h00000009, 128'h00000009000000090000000900000009);
   cn = 2406; testcase_sv(v128_load32_splat,        32'h5555aaaa, 128'h5555aaaa5555aaaa5555aaaa5555aaaa);
   cn = 2407; testcase_sv(v128_load32_splat,        32'h3333cccc, 128'h3333cccc3333cccc3333cccc3333cccc);
   cn = 2408; testcase_sv(v128_load32_splat,        32'h76543210, 128'h76543210765432107654321076543210);

   cn = 2409; testcase_sv(i8x16_splat,              32'h00000000, 128'h00000000000000000000000000000000);
   cn = 2410; testcase_sv(i8x16_splat,              32'h00000005, 128'h05050505050505050505050505050505);
   cn = 2411; testcase_sv(i8x16_splat,              32'hfffffffb, 128'hfbfbfbfbfbfbfbfbfbfbfbfbfbfbfbfb);
   cn = 2412; testcase_sv(i8x16_splat,              32'h00000101, 128'h01010101010101010101010101010101);
   cn = 2413; testcase_sv(i8x16_splat,              32'h000000ff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2414; testcase_sv(i8x16_splat,              32'hffffff80, 128'h80808080808080808080808080808080);
   cn = 2415; testcase_sv(i8x16_splat,              32'h0000007f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2416; testcase_sv(i8x16_splat,              32'hffffff7f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2417; testcase_sv(i8x16_splat,              32'h00000080, 128'h80808080808080808080808080808080);
   cn = 2418; testcase_sv(i8x16_splat,              32'h0000ff7f, 128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f);
   cn = 2419; testcase_sv(i8x16_splat,              32'h00000080, 128'h80808080808080808080808080808080);
   cn = 2420; testcase_sv(i8x16_splat,              32'h000000ab, 128'habababababababababababababababab);
   cn = 2421; testcase_sv(i16x8_splat,              32'h00000000, 128'h00000000000000000000000000000000);
   cn = 2422; testcase_sv(i16x8_splat,              32'h00000005, 128'h00050005000500050005000500050005);
   cn = 2423; testcase_sv(i16x8_splat,              32'hfffffffb, 128'hfffbfffbfffbfffbfffbfffbfffbfffb);
   cn = 2424; testcase_sv(i16x8_splat,              32'h00010001, 128'h00010001000100010001000100010001);
   cn = 2425; testcase_sv(i16x8_splat,              32'h0000ffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2426; testcase_sv(i16x8_splat,              32'hffff8000, 128'h80008000800080008000800080008000);
   cn = 2427; testcase_sv(i16x8_splat,              32'h00007fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2428; testcase_sv(i16x8_splat,              32'hffff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2429; testcase_sv(i16x8_splat,              32'h00008000, 128'h80008000800080008000800080008000);
   cn = 2430; testcase_sv(i16x8_splat,              32'hffff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff);
   cn = 2431; testcase_sv(i16x8_splat,              32'h00008000, 128'h80008000800080008000800080008000);
   cn = 2432; testcase_sv(i16x8_splat,              32'h0000abcd, 128'habcdabcdabcdabcdabcdabcdabcdabcd);
   cn = 2433; testcase_sv(i16x8_splat,              32'h00003039, 128'h30393039303930393039303930393039);
   cn = 2434; testcase_sv(i16x8_splat,              32'h00001234, 128'h12341234123412341234123412341234);
   cn = 2435; testcase_sv(i32x4_splat,              32'h00000000, 128'h00000000000000000000000000000000);
   cn = 2436; testcase_sv(i32x4_splat,              32'h00000005, 128'h00000005000000050000000500000005);
   cn = 2437; testcase_sv(i32x4_splat,              32'hfffffffb, 128'hfffffffbfffffffbfffffffbfffffffb);
   cn = 2438; testcase_sv(i32x4_splat,              32'hffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2439; testcase_sv(i32x4_splat,              32'hffffffff, 128'hffffffffffffffffffffffffffffffff);
   cn = 2440; testcase_sv(i32x4_splat,              32'h80000000, 128'h80000000800000008000000080000000);
   cn = 2441; testcase_sv(i32x4_splat,              32'h7fffffff, 128'h7fffffff7fffffff7fffffff7fffffff);
   cn = 2442; testcase_sv(i32x4_splat,              32'h80000000, 128'h80000000800000008000000080000000);
   cn = 2443; testcase_sv(i32x4_splat,              32'h499602d2, 128'h499602d2499602d2499602d2499602d2);
   cn = 2444; testcase_sv(i32x4_splat,              32'h12345678, 128'h12345678123456781234567812345678);

   $display(" Testing  extadd_pairwise");
   cn = 2445; testpair_vv8( i16x8_extadd_pairwise_i8x16_s,        128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 2446; testpair_vv8( i16x8_extadd_pairwise_i8x16_s,        128'h01010101010101010101010101010101, 128'h00020002000200020002000200020002);
   cn = 2447; testpair_vv8( i16x8_extadd_pairwise_i8x16_s,        128'hffffffffffffffffffffffffffffffff, 128'hfffefffefffefffefffefffefffefffe);
   cn = 2448; testpair_vv8( i16x8_extadd_pairwise_i8x16_s,        128'h7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e, 128'h00fc00fc00fc00fc00fc00fc00fc00fc);
   cn = 2449; testpair_vv8( i16x8_extadd_pairwise_i8x16_s,        128'h81818181818181818181818181818181, 128'hff02ff02ff02ff02ff02ff02ff02ff02);
   cn = 2450; testpair_vv8( i16x8_extadd_pairwise_i8x16_s,        128'h80808080808080808080808080808080, 128'hff00ff00ff00ff00ff00ff00ff00ff00);
   cn = 2451; testpair_vv8( i16x8_extadd_pairwise_i8x16_s,        128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h00fe00fe00fe00fe00fe00fe00fe00fe);
   cn = 2452; testpair_vv8( i16x8_extadd_pairwise_i8x16_s,        128'hffffffffffffffffffffffffffffffff, 128'hfffefffefffefffefffefffefffefffe);
   cn = 2453; testpair_vv8( i16x8_extadd_pairwise_i8x16_u,        128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 2454; testpair_vv8( i16x8_extadd_pairwise_i8x16_u,        128'h01010101010101010101010101010101, 128'h00020002000200020002000200020002);
   cn = 2455; testpair_vv8( i16x8_extadd_pairwise_i8x16_u,        128'hffffffffffffffffffffffffffffffff, 128'h01fe01fe01fe01fe01fe01fe01fe01fe);
   cn = 2456; testpair_vv8( i16x8_extadd_pairwise_i8x16_u,        128'h7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e, 128'h00fc00fc00fc00fc00fc00fc00fc00fc);
   cn = 2457; testpair_vv8( i16x8_extadd_pairwise_i8x16_u,        128'h81818181818181818181818181818181, 128'h01020102010201020102010201020102);
   cn = 2458; testpair_vv8( i16x8_extadd_pairwise_i8x16_u,        128'h80808080808080808080808080808080, 128'h01000100010001000100010001000100);
   cn = 2459; testpair_vv8( i16x8_extadd_pairwise_i8x16_u,        128'h7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f, 128'h00fe00fe00fe00fe00fe00fe00fe00fe);
   cn = 2460; testpair_vv8( i16x8_extadd_pairwise_i8x16_u,        128'hffffffffffffffffffffffffffffffff, 128'h01fe01fe01fe01fe01fe01fe01fe01fe);
   cn = 2461; testpair_vv16(i32x4_extadd_pairwise_i16x8_s,        128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 2462; testpair_vv16(i32x4_extadd_pairwise_i16x8_s,        128'h00010001000100010001000100010001, 128'h00000002000000020000000200000002);
   cn = 2463; testpair_vv16(i32x4_extadd_pairwise_i16x8_s,        128'hffffffffffffffffffffffffffffffff, 128'hfffffffefffffffefffffffefffffffe);
   cn = 2464; testpair_vv16(i32x4_extadd_pairwise_i16x8_s,        128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe, 128'h0000fffc0000fffc0000fffc0000fffc);
   cn = 2465; testpair_vv16(i32x4_extadd_pairwise_i16x8_s,        128'h80018001800180018001800180018001, 128'hffff0002ffff0002ffff0002ffff0002);
   cn = 2466; testpair_vv16(i32x4_extadd_pairwise_i16x8_s,        128'h80008000800080008000800080008000, 128'hffff0000ffff0000ffff0000ffff0000);
   cn = 2467; testpair_vv16(i32x4_extadd_pairwise_i16x8_s,        128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h0000fffe0000fffe0000fffe0000fffe);
   cn = 2468; testpair_vv16(i32x4_extadd_pairwise_i16x8_s,        128'hffffffffffffffffffffffffffffffff, 128'hfffffffefffffffefffffffefffffffe);
   cn = 2469; testpair_vv16(i32x4_extadd_pairwise_i16x8_u,        128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 2470; testpair_vv16(i32x4_extadd_pairwise_i16x8_u,        128'h00010001000100010001000100010001, 128'h00000002000000020000000200000002);
   cn = 2471; testpair_vv16(i32x4_extadd_pairwise_i16x8_u,        128'hffffffffffffffffffffffffffffffff, 128'h0001fffe0001fffe0001fffe0001fffe);
   cn = 2472; testpair_vv16(i32x4_extadd_pairwise_i16x8_u,        128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe, 128'h0000fffc0000fffc0000fffc0000fffc);
   cn = 2473; testpair_vv16(i32x4_extadd_pairwise_i16x8_u,        128'h80018001800180018001800180018001, 128'h00010002000100020001000200010002);
   cn = 2474; testpair_vv16(i32x4_extadd_pairwise_i16x8_u,        128'h80008000800080008000800080008000, 128'h00010000000100000001000000010000);
   cn = 2475; testpair_vv16(i32x4_extadd_pairwise_i16x8_u,        128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h0000fffe0000fffe0000fffe0000fffe);
   cn = 2476; testpair_vv16(i32x4_extadd_pairwise_i16x8_u,        128'hffffffffffffffffffffffffffffffff, 128'h0001fffe0001fffe0001fffe0001fffe);

   $display(" Testing  extmul");
   cn = 2477; testcase_vv2v(i16x8_extmul_dbl_i8x16_s, 128'h00_00_00_00_00_00_00_00_00_00_00_00_00_00_00_00, 128'h00_00_00_00_00_00_00_00_00_00_00_00_00_00_00_00, 128'h0000_0000_0000_0000_0000_0000_0000_0000, 128'h0000_0000_0000_0000_0000_0000_0000_0000);
   cn = 2478; testcase_vv2v(i16x8_extmul_dbl_i8x16_s, 128'h80_c0_e0_f0_f8_fc_fe_ff_7f_3f_1f_0f_07_03_01_00, 128'h80_c0_e0_f0_f8_fc_fe_ff_7f_3f_1f_0f_07_03_01_00, 128'h3f01_0f81_03c1_00e1_0031_0009_0001_0000, 128'h4000_1000_0400_0100_0040_0010_0004_0001);
   cn = 2479; testcase_vv2v(i32x4_extmul_dbl_i16x8_s, 128'h0000_0000_0000_0000_0000_0000_0000_0000,         128'h0000_0000_0000_0000_0000_0000_0000_0000,         128'h00000000_00000000_00000000_00000000,     128'h00000000_00000000_00000000_00000000);
   cn = 2480; testcase_vv2v(i32x4_extmul_dbl_i16x8_s, 128'h8000_e000_f800_fe00_7f00_1f00_0700_0100,         128'h8000_e000_f800_fe00_7f00_1f00_0700_0100,         128'h3f010000_03c10000_00310000_00010000,     128'h40000000_04000000_00400000_00040000);
   cn = 2481; testcase_vv2v(i16x8_extmul_dbl_i8x16_u, 128'h00_00_00_00_00_00_00_00_00_00_00_00_00_00_00_00, 128'h00_00_00_00_00_00_00_00_00_00_00_00_00_00_00_00, 128'h0000_0000_0000_0000_0000_0000_0000_0000, 128'h0000_0000_0000_0000_0000_0000_0000_0000);
   cn = 2482; testcase_vv2v(i16x8_extmul_dbl_i8x16_u, 128'h80_c0_e0_f0_f8_fc_fe_ff_7f_3f_1f_0f_07_03_01_00, 128'h80_c0_e0_f0_f8_fc_fe_ff_7f_3f_1f_0f_07_03_01_00, 128'h3f01_0f81_03c1_00e1_0031_0009_0001_0000, 128'h4000_9000_c400_e100_f040_f810_fc04_fe01);
   cn = 2483; testcase_vv2v(i32x4_extmul_dbl_i16x8_u, 128'h0000_0000_0000_0000_0000_0000_0000_0000,         128'h0000_0000_0000_0000_0000_0000_0000_0000,         128'h00000000_00000000_00000000_00000000,     128'h00000000_00000000_00000000_00000000);
   cn = 2484; testcase_vv2v(i32x4_extmul_dbl_i16x8_u, 128'h8000_e000_f800_fe00_7f00_1f00_0700_0100,         128'h8000_e000_f800_fe00_7f00_1f00_0700_0100,         128'h3f010000_03c10000_00310000_00010000,     128'h40000000_c4000000_f0400000_fc040000);

   $display(" Testing  extend");
   cn = 2485; testcase_xd8s( i16x8_extend_dbl_i8x16_s,  128'hf0_e0_d0_c0_b0_a0_90_80_70_60_50_40_30_20_10_00, 128'h0070_0060_0050_0040_0030_0020_0010_0000,    128'hfff0_ffe0_ffd0_ffc0_ffb0_ffa0_ff90_ff80);

   cn = 2486; testcase_xd16s(i32x4_extend_dbl_i16x8_s,  128'h8000_e000_f800_fe00_7f00_1f00_0700_0100,         128'h00007f00_00001f00_00000700_00000100,        128'hffff8000_ffffe000_fffff800_fffffe00);

   cn = 2487; testcase_xd8u( i16x8_extend_dbl_i8x16_u,  128'hf0_e0_d0_c0_b0_a0_90_80_70_60_50_40_30_20_10_00, 128'h0070_0060_0050_0040_0030_0020_0010_0000,    128'h00f0_00e0_00d0_00c0_00b0_00a0_0090_0080);

   cn = 2488; testcase_xd16u(i32x4_extend_dbl_i16x8_u,  128'h8000_e000_f800_fe00_7f00_1f00_0700_0100,         128'h00007f00_00001f00_00000700_00000100,        128'h00008000_0000e000_0000f800_0000fe00);

// v128_load8x8_s       \
// v128_load16x4_s       \___ Load followed by extend:  Transsembler problem!
// v128_load8x8_u        /
// v128_load16x4_u      /

// v128_load8_lane      \
// v128_load16_lane      \___ Load scalar followed load lane from scalar - immediate lane specifier in opcode
// v128_load32_lane     _/

   $display(" Testing  v128_loadx_lane");
   cn = 2490; testcase_sivv(v128_load8_lane,  4'h0, 32'h0000000f, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_0f);
   cn = 2491; testcase_sivv(v128_load8_lane,  4'h1, 32'h0000000e, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_0e_ff);
   cn = 2492; testcase_sivv(v128_load8_lane,  4'h2, 32'h0000000d, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_0d_ff_ff);
   cn = 2493; testcase_sivv(v128_load8_lane,  4'h3, 32'h0000000c, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_0c_ff_ff_ff);
   cn = 2494; testcase_sivv(v128_load8_lane,  4'h4, 32'h0000000b, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_0b_ff_ff_ff_ff);
   cn = 2495; testcase_sivv(v128_load8_lane,  4'h5, 32'h0000000a, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_0a_ff_ff_ff_ff_ff);
   cn = 2496; testcase_sivv(v128_load8_lane,  4'h6, 32'h00000009, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_09_ff_ff_ff_ff_ff_ff);
   cn = 2497; testcase_sivv(v128_load8_lane,  4'h7, 32'h00000008, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff, 128'hff_ff_ff_ff_ff_ff_ff_ff_08_ff_ff_ff_ff_ff_ff_ff);
   cn = 2498; testcase_sivv(v128_load8_lane,  4'h8, 32'h00000007, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff, 128'hff_ff_ff_ff_ff_ff_ff_07_ff_ff_ff_ff_ff_ff_ff_ff);
   cn = 2499; testcase_sivv(v128_load8_lane,  4'h9, 32'h00000006, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff, 128'hff_ff_ff_ff_ff_ff_06_ff_ff_ff_ff_ff_ff_ff_ff_ff);
   cn = 2500; testcase_sivv(v128_load8_lane,  4'ha, 32'h00000005, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff, 128'hff_ff_ff_ff_ff_05_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff);
   cn = 2501; testcase_sivv(v128_load8_lane,  4'hb, 32'h00000004, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff, 128'hff_ff_ff_ff_04_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff);
   cn = 2502; testcase_sivv(v128_load8_lane,  4'hc, 32'h00000003, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff, 128'hff_ff_ff_03_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff);
   cn = 2503; testcase_sivv(v128_load8_lane,  4'hd, 32'h00000002, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff, 128'hff_ff_02_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff);
   cn = 2504; testcase_sivv(v128_load8_lane,  4'he, 32'h00000001, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff, 128'hff_01_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff);
   cn = 2505; testcase_sivv(v128_load8_lane,  4'hf, 32'h00000000, 128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff, 128'h00_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff);

   cn = 2506; testcase_sivv(v128_load16_lane, 4'h0, 32'h0000cdef, 128'hffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff, 128'hffff_ffff_ffff_ffff_ffff_ffff_ffff_cdef);
   cn = 2507; testcase_sivv(v128_load16_lane, 4'h1, 32'h0000bcde, 128'hffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff, 128'hffff_ffff_ffff_ffff_ffff_ffff_bcde_ffff);
   cn = 2508; testcase_sivv(v128_load16_lane, 4'h2, 32'h0000abcd, 128'hffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff, 128'hffff_ffff_ffff_ffff_ffff_abcd_ffff_ffff);
   cn = 2509; testcase_sivv(v128_load16_lane, 4'h3, 32'h00009abc, 128'hffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff, 128'hffff_ffff_ffff_ffff_9abc_ffff_ffff_ffff);
   cn = 2510; testcase_sivv(v128_load16_lane, 4'h4, 32'h000089ab, 128'hffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff, 128'hffff_ffff_ffff_89ab_ffff_ffff_ffff_ffff);
   cn = 2511; testcase_sivv(v128_load16_lane, 4'h5, 32'h0000789a, 128'hffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff, 128'hffff_ffff_789a_ffff_ffff_ffff_ffff_ffff);
   cn = 2512; testcase_sivv(v128_load16_lane, 4'h6, 32'h00006789, 128'hffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff, 128'hffff_6789_ffff_ffff_ffff_ffff_ffff_ffff);
   cn = 2513; testcase_sivv(v128_load16_lane, 4'h7, 32'h00005678, 128'hffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff, 128'h5678_ffff_ffff_ffff_ffff_ffff_ffff_ffff);

   cn = 2514; testcase_sivv(v128_load32_lane, 4'h0, 32'h89abcdef, 128'hffffffff_ffffffff_ffffffff_ffffffff, 128'hffffffff_ffffffff_ffffffff_89abcdef);
   cn = 2515; testcase_sivv(v128_load32_lane, 4'h1, 32'h01234567, 128'hffffffff_ffffffff_ffffffff_ffffffff, 128'hffffffff_ffffffff_01234567_ffffffff);
   cn = 2516; testcase_sivv(v128_load32_lane, 4'h2, 32'hfedcba98, 128'hffffffff_ffffffff_ffffffff_ffffffff, 128'hffffffff_fedcba98_ffffffff_ffffffff);
   cn = 2517; testcase_sivv(v128_load32_lane, 4'h3, 32'h76543210, 128'hffffffff_ffffffff_ffffffff_ffffffff, 128'h76543210_ffffffff_ffffffff_ffffffff);

// i8x16_replace_lane   // These are not needed as v128_loadxx_lane do the same thing except they must first load T0 whereas here,
// i16x8_replace_lane   // T0 is assumed loaded. So its a Transsembler thing!
// i32x4_replace_lane

// i8x16_extract_lane_s // Extracted size is sign extended to 32-bit scalar
// i16x8_extract_lane_s
// i8x16_extract_lane_u // Extracted size is zero extended to 32-bit scalar
// i16x8_extract_lane_u
// i32x4_extract_lane   // Extracted size is a 32-bit scalar

   $display(" Testing  v128_extract_lane");
   cn = 2518; testcase_ivs(i8x16_extract_lane_s, 4'h0, 128'h00_00_00_00_00_00_00_00_00_00_00_00_00_00_00_f0, 32'hfffffff0);
   cn = 2519; testcase_ivs(i8x16_extract_lane_s, 4'h1, 128'h00_00_00_00_00_00_00_00_00_00_00_00_00_00_11_00, 32'h00000011);
   cn = 2520; testcase_ivs(i8x16_extract_lane_s, 4'h2, 128'h00_00_00_00_00_00_00_00_00_00_00_00_00_22_00_00, 32'h00000022);
   cn = 2521; testcase_ivs(i8x16_extract_lane_s, 4'h3, 128'h00_00_00_00_00_00_00_00_00_00_00_00_33_00_00_00, 32'h00000033);
   cn = 2522; testcase_ivs(i8x16_extract_lane_s, 4'h4, 128'h00_00_00_00_00_00_00_00_00_00_00_44_00_00_00_00, 32'h00000044);
   cn = 2523; testcase_ivs(i8x16_extract_lane_s, 4'h5, 128'h00_00_00_00_00_00_00_00_00_00_55_00_00_00_00_00, 32'h00000055);
   cn = 2524; testcase_ivs(i8x16_extract_lane_s, 4'h6, 128'h00_00_00_00_00_00_00_00_00_66_00_00_00_00_00_00, 32'h00000066);
   cn = 2525; testcase_ivs(i8x16_extract_lane_s, 4'h7, 128'h00_00_00_00_00_00_00_00_77_00_00_00_00_00_00_00, 32'h00000077);
   cn = 2526; testcase_ivs(i8x16_extract_lane_s, 4'h8, 128'h00_00_00_00_00_00_00_88_00_00_00_00_00_00_00_00, 32'hffffff88);
   cn = 2527; testcase_ivs(i8x16_extract_lane_s, 4'h9, 128'h00_00_00_00_00_00_99_00_00_00_00_00_00_00_00_00, 32'hffffff99);
   cn = 2528; testcase_ivs(i8x16_extract_lane_s, 4'ha, 128'h00_00_00_00_00_aa_00_00_00_00_00_00_00_00_00_00, 32'hffffffaa);
   cn = 2529; testcase_ivs(i8x16_extract_lane_s, 4'hb, 128'h00_00_00_00_bb_00_00_00_00_00_00_00_00_00_00_00, 32'hffffffbb);
   cn = 2530; testcase_ivs(i8x16_extract_lane_s, 4'hc, 128'h00_00_00_cc_00_00_00_00_00_00_00_00_00_00_00_00, 32'hffffffcc);
   cn = 2531; testcase_ivs(i8x16_extract_lane_s, 4'hd, 128'h00_00_dd_00_00_00_00_00_00_00_00_00_00_00_00_00, 32'hffffffdd);
   cn = 2532; testcase_ivs(i8x16_extract_lane_s, 4'he, 128'h00_ee_00_00_00_00_00_00_00_00_00_00_00_00_00_00, 32'hffffffee);
   cn = 2533; testcase_ivs(i8x16_extract_lane_s, 4'hf, 128'hff_00_00_00_00_00_00_00_00_00_00_00_00_00_00_00, 32'hffffffff);

   cn = 2534; testcase_ivs(i16x8_extract_lane_s, 4'h0, 128'h0000_0000_0000_0000_0000_0000_0000_1100, 32'h00001100);
   cn = 2535; testcase_ivs(i16x8_extract_lane_s, 4'h1, 128'h0000_0000_0000_0000_0000_0000_3322_0000, 32'h00003322);
   cn = 2536; testcase_ivs(i16x8_extract_lane_s, 4'h2, 128'h0000_0000_0000_0000_0000_5544_0000_0000, 32'h00005544);
   cn = 2537; testcase_ivs(i16x8_extract_lane_s, 4'h3, 128'h0000_0000_0000_0000_7766_0000_0000_0000, 32'h00007766);
   cn = 2538; testcase_ivs(i16x8_extract_lane_s, 4'h4, 128'h0000_0000_0000_9988_0000_0000_0000_0000, 32'hffff9988);
   cn = 2539; testcase_ivs(i16x8_extract_lane_s, 4'h5, 128'h0000_0000_bbaa_0000_0000_0000_0000_0000, 32'hffffbbaa);
   cn = 2540; testcase_ivs(i16x8_extract_lane_s, 4'h6, 128'h0000_ddcc_0000_0000_0000_0000_0000_0000, 32'hffffddcc);
   cn = 2541; testcase_ivs(i16x8_extract_lane_s, 4'h7, 128'hffee_0000_0000_0000_0000_0000_0000_0000, 32'hffffffee);

   cn = 2542; testcase_ivs(i8x16_extract_lane_u, 4'h0, 128'h00_00_00_00_00_00_00_00_00_00_00_00_00_00_00_f0, 32'h000000f0);
   cn = 2543; testcase_ivs(i8x16_extract_lane_u, 4'h1, 128'h00_00_00_00_00_00_00_00_00_00_00_00_00_00_11_00, 32'h00000011);
   cn = 2544; testcase_ivs(i8x16_extract_lane_u, 4'h2, 128'h00_00_00_00_00_00_00_00_00_00_00_00_00_22_00_00, 32'h00000022);
   cn = 2545; testcase_ivs(i8x16_extract_lane_u, 4'h3, 128'h00_00_00_00_00_00_00_00_00_00_00_00_33_00_00_00, 32'h00000033);
   cn = 2546; testcase_ivs(i8x16_extract_lane_u, 4'h4, 128'h00_00_00_00_00_00_00_00_00_00_00_44_00_00_00_00, 32'h00000044);
   cn = 2547; testcase_ivs(i8x16_extract_lane_u, 4'h5, 128'h00_00_00_00_00_00_00_00_00_00_55_00_00_00_00_00, 32'h00000055);
   cn = 2548; testcase_ivs(i8x16_extract_lane_u, 4'h6, 128'h00_00_00_00_00_00_00_00_00_66_00_00_00_00_00_00, 32'h00000066);
   cn = 2549; testcase_ivs(i8x16_extract_lane_u, 4'h7, 128'h00_00_00_00_00_00_00_00_77_00_00_00_00_00_00_00, 32'h00000077);
   cn = 2550; testcase_ivs(i8x16_extract_lane_u, 4'h8, 128'h00_00_00_00_00_00_00_88_00_00_00_00_00_00_00_00, 32'h00000088);
   cn = 2551; testcase_ivs(i8x16_extract_lane_u, 4'h9, 128'h00_00_00_00_00_00_99_00_00_00_00_00_00_00_00_00, 32'h00000099);
   cn = 2552; testcase_ivs(i8x16_extract_lane_u, 4'ha, 128'h00_00_00_00_00_aa_00_00_00_00_00_00_00_00_00_00, 32'h000000aa);
   cn = 2553; testcase_ivs(i8x16_extract_lane_u, 4'hb, 128'h00_00_00_00_bb_00_00_00_00_00_00_00_00_00_00_00, 32'h000000bb);
   cn = 2554; testcase_ivs(i8x16_extract_lane_u, 4'hc, 128'h00_00_00_cc_00_00_00_00_00_00_00_00_00_00_00_00, 32'h000000cc);
   cn = 2555; testcase_ivs(i8x16_extract_lane_u, 4'hd, 128'h00_00_dd_00_00_00_00_00_00_00_00_00_00_00_00_00, 32'h000000dd);
   cn = 2556; testcase_ivs(i8x16_extract_lane_u, 4'he, 128'h00_ee_00_00_00_00_00_00_00_00_00_00_00_00_00_00, 32'h000000ee);
   cn = 2557; testcase_ivs(i8x16_extract_lane_u, 4'hf, 128'h0f_00_00_00_00_00_00_00_00_00_00_00_00_00_00_00, 32'h0000000f);

   cn = 2558; testcase_ivs(i16x8_extract_lane_u, 4'h0, 128'h0000_0000_0000_0000_0000_0000_0000_eeff, 32'h0000eeff);
   cn = 2559; testcase_ivs(i16x8_extract_lane_u, 4'h1, 128'h0000_0000_0000_0000_0000_0000_ccdd_0000, 32'h0000ccdd);
   cn = 2560; testcase_ivs(i16x8_extract_lane_u, 4'h2, 128'h0000_0000_0000_0000_0000_aabb_0000_0000, 32'h0000aabb);
   cn = 2561; testcase_ivs(i16x8_extract_lane_u, 4'h3, 128'h0000_0000_0000_0000_8899_0000_0000_0000, 32'h00008899);
   cn = 2562; testcase_ivs(i16x8_extract_lane_u, 4'h4, 128'h0000_0000_0000_6677_0000_0000_0000_0000, 32'h00006677);
   cn = 2563; testcase_ivs(i16x8_extract_lane_u, 4'h5, 128'h0000_0000_4455_0000_0000_0000_0000_0000, 32'h00004455);
   cn = 2564; testcase_ivs(i16x8_extract_lane_u, 4'h6, 128'h0000_2233_0000_0000_0000_0000_0000_0000, 32'h00002233);
   cn = 2565; testcase_ivs(i16x8_extract_lane_u, 4'h7, 128'h0011_0000_0000_0000_0000_0000_0000_0000, 32'h00000011);

   cn = 2566; testcase_ivs(i32x4_extract_lane, 4'h0, 128'h00000000_00000000_00000000_11111111, 32'h11111111);
   cn = 2567; testcase_ivs(i32x4_extract_lane, 4'h1, 128'h00000000_00000000_22222222_00000000, 32'h22222222);
   cn = 2568; testcase_ivs(i32x4_extract_lane, 4'h2, 128'h00000000_33333333_00000000_00000000, 32'h33333333);
   cn = 2569; testcase_ivs(i32x4_extract_lane, 4'h3, 128'h44444444_00000000_00000000_00000000, 32'h44444444);

// Spot check
   $display(" Testing  spot checks");
   cn = 2570; testcase_vvv(i8x16_eq,   128'hff_ee_dd_cc_bb_aa_99_88_77_66_55_44_33_22_11_00,       // a
                                       128'hff_ee_dd_cc_bb_aa_99_88_77_66_55_44_33_22_11_00,       // b
                                       128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff );     // c = a == b ? ffff : 0000
   cn = 2571; testcase_vvv(i8x16_eq,   128'hff_ee_dd_cc_bb_aa_99_88_77_66_55_44_33_22_11_00,       // a
                                       128'hfe_ec_d9_c4_ab_8a_b9_08_74_60_50_00_22_11_00_cc,       // b
                                       128'h00_00_00_00_00_00_00_00_00_00_00_00_00_00_00_00 );     // c = a == b ? ffff : 0000

   cn = 2574; testcase_vvv(i8x16_lt_s, 128'hff_ee_dd_cc_bb_aa_99_88_77_66_55_44_33_22_11_00,       // a
                                       128'hff_ee_dd_cc_bb_aa_99_88_77_66_55_44_33_22_11_00,       // b
                                       128'h00_00_00_00_00_00_00_00_00_00_00_00_00_00_00_00 );     // c = a <  b ? ffff : 0000

   cn = 2575; testcase_vvv(i8x16_lt_s, 128'hb9_08_dd_cc_bb_aa_ff_ee_77_66_55_44_33_22_11_00,       // a
                                       128'h99_88_d9_c4_ab_8a_fe_ec_74_60_50_00_22_11_00_cc,       // b
                                       128'h00_00_00_00_00_00_00_00_00_00_00_00_00_00_00_00 );     // c = a <  b ? ffff : 0000

   cn = 2576; testcase_vvv(i8x16_lt_s, 128'hfe_ec_d9_c4_ab_8a_99_88_74_60_50_00_22_11_00_cc,       // a
                                       128'hff_ee_dd_cc_bb_aa_b9_08_77_66_55_44_33_22_11_00,       // b
                                       128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff );     // c = a <  b ? ffff : 0000

   `ifdef   ALL_RELATIONALS
   cn = 2572; testcase_vvv(i8x16_ne,   128'hff_ee_dd_cc_bb_aa_99_88_77_66_55_44_33_22_11_00,       // a
                                       128'hff_ee_dd_cc_bb_aa_99_88_77_66_55_44_33_22_11_00,       // b
                                       128'h00_00_00_00_00_00_00_00_00_00_00_00_00_00_00_00 );     // c = a != b ? ffff : 0000
   cn = 2573; testcase_vvv(i8x16_ne,   128'hff_ee_dd_cc_bb_aa_99_88_77_66_55_44_33_22_11_00,       // a
                                       128'hfe_ec_d9_c4_ab_8a_b9_08_74_60_50_00_22_11_00_cc,       // b
                                       128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff );     // c = a != b ? ffff : 0000

   cn = 2577; testcase_vvv(i8x16_ge_s, 128'hff_ee_dd_cc_bb_aa_99_88_77_66_55_44_33_22_11_00,       // a
                                       128'hff_ee_dd_cc_bb_aa_99_88_77_66_55_44_33_22_11_00,       // b
                                       128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff );     // c = a >= b ? ffff : 0000

   cn = 2578; testcase_vvv(i8x16_ge_s, 128'hff_ee_dd_cc_bb_aa_b9_08_77_66_55_44_33_22_11_00,       // a
                                       128'hfe_ec_d9_c4_ab_8a_99_88_74_60_50_00_22_11_00_cc,       // b
                                       128'hff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff_ff );     // c = a >= b ? ffff : 0000

   cn = 2579; testcase_vvv(i8x16_ge_s, 128'hfe_ec_d9_c4_ab_8a_99_88_74_60_50_00_22_11_00_cc,       // a
                                       128'hff_ee_dd_cc_bb_aa_b9_08_77_66_55_44_33_22_11_00,       // b
                                       128'h00_00_00_00_00_00_00_00_00_00_00_00_00_00_00_00 );     // c = a >= b ? ffff : 0000
   `endif //ALL_RELATIONALS
   `ifdef   ENABLE_DOT_PRODUCT
   $display(" Testing  dot product");
   cn = 2580; testcase_dot(i32x4_dot_i16x8_s,   128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 2581; testcase_dot(i32x4_dot_i16x8_s,   128'h00000000000000000000000000000000, 128'h00010001000100010001000100010001, 128'h00000000000000000000000000000000);

   cn = 2582; testcase_dot(i32x4_dot_i16x8_s,   128'h00010001000100010001000100010001, 128'h00010001000100010001000100010001, 128'h00000002000000020000000200000002);

   cn = 2583; testcase_dot(i32x4_dot_i16x8_s,   128'h00000000000000000000000000000000, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000);
   cn = 2584; testcase_dot(i32x4_dot_i16x8_s,   128'h00010001000100010001000100010001, 128'hffffffffffffffffffffffffffffffff, 128'hfffffffefffffffefffffffefffffffe);
   cn = 2585; testcase_dot(i32x4_dot_i16x8_s,   128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000002000000020000000200000002);
   cn = 2586; testcase_dot(i32x4_dot_i16x8_s,   128'h3fff3fff3fff3fff3fff3fff3fff3fff, 128'h40004000400040004000400040004000, 128'h1fff80001fff80001fff80001fff8000);
   cn = 2587; testcase_dot(i32x4_dot_i16x8_s,   128'h40004000400040004000400040004000, 128'h40004000400040004000400040004000, 128'h20000000200000002000000020000000);
   cn = 2588; testcase_dot(i32x4_dot_i16x8_s,   128'hc001c001c001c001c001c001c001c001, 128'hc000c000c000c000c000c000c000c000, 128'h1fff80001fff80001fff80001fff8000);
   cn = 2589; testcase_dot(i32x4_dot_i16x8_s,   128'hc000c000c000c000c000c000c000c000, 128'hc000c000c000c000c000c000c000c000, 128'h20000000200000002000000020000000);
   cn = 2590; testcase_dot(i32x4_dot_i16x8_s,   128'hbfffbfffbfffbfffbfffbfffbfffbfff, 128'hc000c000c000c000c000c000c000c000, 128'h20008000200080002000800020008000);
   cn = 2591; testcase_dot(i32x4_dot_i16x8_s,   128'h7ffd7ffd7ffd7ffd7ffd7ffd7ffd7ffd, 128'h00010001000100010001000100010001, 128'h0000fffa0000fffa0000fffa0000fffa);
   cn = 2592; testcase_dot(i32x4_dot_i16x8_s,   128'h7ffe7ffe7ffe7ffe7ffe7ffe7ffe7ffe, 128'h00010001000100010001000100010001, 128'h0000fffc0000fffc0000fffc0000fffc);
   cn = 2593; testcase_dot(i32x4_dot_i16x8_s,   128'h80008000800080008000800080008000, 128'h00010001000100010001000100010001, 128'hffff0000ffff0000ffff0000ffff0000);
   cn = 2594; testcase_dot(i32x4_dot_i16x8_s,   128'h80028002800280028002800280028002, 128'hffffffffffffffffffffffffffffffff, 128'h0000fffc0000fffc0000fffc0000fffc);
   cn = 2595; testcase_dot(i32x4_dot_i16x8_s,   128'h80018001800180018001800180018001, 128'hffffffffffffffffffffffffffffffff, 128'h0000fffe0000fffe0000fffe0000fffe);
   cn = 2596; testcase_dot(i32x4_dot_i16x8_s,   128'h80008000800080008000800080008000, 128'hffffffffffffffffffffffffffffffff, 128'h00010000000100000001000000010000);
   cn = 2597; testcase_dot(i32x4_dot_i16x8_s,   128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'h7ffe00027ffe00027ffe00027ffe0002);
   cn = 2598; testcase_dot(i32x4_dot_i16x8_s,   128'h80008000800080008000800080008000, 128'h80008000800080008000800080008000, 128'h80000000800000008000000080000000);
   cn = 2599; testcase_dot(i32x4_dot_i16x8_s,   128'h80008000800080008000800080008000, 128'h80018001800180018001800180018001, 128'h7fff00007fff00007fff00007fff0000);
   cn = 2600; testcase_dot(i32x4_dot_i16x8_s,   128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 2601; testcase_dot(i32x4_dot_i16x8_s,   128'hffffffffffffffffffffffffffffffff, 128'h00010001000100010001000100010001, 128'hfffffffefffffffefffffffefffffffe);
   cn = 2602; testcase_dot(i32x4_dot_i16x8_s,   128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000002000000020000000200000002);
   cn = 2603; testcase_dot(i32x4_dot_i16x8_s,   128'hffffffffffffffffffffffffffffffff, 128'h7fff7fff7fff7fff7fff7fff7fff7fff, 128'hffff0002ffff0002ffff0002ffff0002);
   cn = 2604; testcase_dot(i32x4_dot_i16x8_s,   128'hffffffffffffffffffffffffffffffff, 128'h80008000800080008000800080008000, 128'h00010000000100000001000000010000);
   cn = 2605; testcase_dot(i32x4_dot_i16x8_s,   128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'h00000002000000020000000200000002);
   `endif //ENABLE_DOT_PRODUCT
   //                                   Address Prime with                             Write test data                        Expected result
   $display(" Testing  v128_store");
   cn = 2606; teststor_svvv(v128_store, 32'h00, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 2607; teststor_svvv(v128_store, 32'h10, 128'hffffffffffffffffffffffffffffffff, 128'h11111111111111111111111111111111, 128'h11111111111111111111111111111111);
   cn = 2608; teststor_svvv(v128_store, 32'h20, 128'hffffffffffffffffffffffffffffffff, 128'h22222222222222222222222222222222, 128'h22222222222222222222222222222222);
   cn = 2609; teststor_svvv(v128_store, 32'h30, 128'hffffffffffffffffffffffffffffffff, 128'h33333333333333333333333333333333, 128'h33333333333333333333333333333333);
   cn = 2610; teststor_svvv(v128_store, 32'h40, 128'hffffffffffffffffffffffffffffffff, 128'h44444444444444444444444444444444, 128'h44444444444444444444444444444444);
   cn = 2611; teststor_svvv(v128_store, 32'h50, 128'hffffffffffffffffffffffffffffffff, 128'h55555555555555555555555555555555, 128'h55555555555555555555555555555555);
   cn = 2612; teststor_svvv(v128_store, 32'h60, 128'hffffffffffffffffffffffffffffffff, 128'h66666666666666666666666666666666, 128'h66666666666666666666666666666666);
   cn = 2613; teststor_svvv(v128_store, 32'h70, 128'hffffffffffffffffffffffffffffffff, 128'h77777777777777777777777777777777, 128'h77777777777777777777777777777777);
   cn = 2614; teststor_svvv(v128_store, 32'h80, 128'hffffffffffffffffffffffffffffffff, 128'h88888888888888888888888888888888, 128'h88888888888888888888888888888888);
   cn = 2615; teststor_svvv(v128_store, 32'h90, 128'hffffffffffffffffffffffffffffffff, 128'h99999999999999999999999999999999, 128'h99999999999999999999999999999999);
   cn = 2616; teststor_svvv(v128_store, 32'ha0, 128'hffffffffffffffffffffffffffffffff, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, 128'haaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
   cn = 2617; teststor_svvv(v128_store, 32'hb0, 128'hffffffffffffffffffffffffffffffff, 128'hbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, 128'hbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
   cn = 2618; teststor_svvv(v128_store, 32'hc0, 128'hffffffffffffffffffffffffffffffff, 128'hcccccccccccccccccccccccccccccccc, 128'hcccccccccccccccccccccccccccccccc);
   cn = 2619; teststor_svvv(v128_store, 32'hd0, 128'hffffffffffffffffffffffffffffffff, 128'hdddddddddddddddddddddddddddddddd, 128'hdddddddddddddddddddddddddddddddd);
   cn = 2620; teststor_svvv(v128_store, 32'he0, 128'hffffffffffffffffffffffffffffffff, 128'heeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee, 128'heeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee);
   cn = 2621; teststor_svvv(v128_store, 32'hf0, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff, 128'hffffffffffffffffffffffffffffffff);

   $display(" Testing  v128_store_lane");
   cn = 2622; teststor_sivvv(v128_store8_lane, 4'h0, 32'h00, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000000000, 128'h00000000000000000000000000000000);
   cn = 2623; teststor_sivvv(v128_store8_lane, 4'h1, 32'h01, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000001100, 128'h00000000000000000000000000001100);
   cn = 2624; teststor_sivvv(v128_store8_lane, 4'h2, 32'h02, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000000220000, 128'h00000000000000000000000000221100);
   cn = 2625; teststor_sivvv(v128_store8_lane, 4'h3, 32'h03, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000000033000000, 128'h00000000000000000000000033221100);
   cn = 2626; teststor_sivvv(v128_store8_lane, 4'h4, 32'h04, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000004400000000, 128'h00000000000000000000004433221100);
   cn = 2627; teststor_sivvv(v128_store8_lane, 4'h5, 32'h05, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000000550000000000, 128'h00000000000000000000554433221100);
   cn = 2628; teststor_sivvv(v128_store8_lane, 4'h6, 32'h06, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000000066000000000000, 128'h00000000000000000066554433221100);
   cn = 2629; teststor_sivvv(v128_store8_lane, 4'h7, 32'h07, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000007700000000000000, 128'h00000000000000007766554433221100);
   cn = 2630; teststor_sivvv(v128_store8_lane, 4'h8, 32'h08, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000000880000000000000000, 128'h00000000000000887766554433221100);
   cn = 2631; teststor_sivvv(v128_store8_lane, 4'h9, 32'h09, 128'hffffffffffffffffffffffffffffffff, 128'h00000000000099000000000000000000, 128'h00000000000099887766554433221100);
   cn = 2632; teststor_sivvv(v128_store8_lane, 4'ha, 32'h0a, 128'hffffffffffffffffffffffffffffffff, 128'h0000000000aa00000000000000000000, 128'h0000000000aa99887766554433221100);
   cn = 2633; teststor_sivvv(v128_store8_lane, 4'hb, 32'h0b, 128'hffffffffffffffffffffffffffffffff, 128'h00000000bb0000000000000000000000, 128'h00000000bbaa99887766554433221100);
   cn = 2634; teststor_sivvv(v128_store8_lane, 4'hc, 32'h0c, 128'hffffffffffffffffffffffffffffffff, 128'h000000cc000000000000000000000000, 128'h000000ccbbaa99887766554433221100);
   cn = 2635; teststor_sivvv(v128_store8_lane, 4'hd, 32'h0d, 128'hffffffffffffffffffffffffffffffff, 128'h0000dd00000000000000000000000000, 128'h0000ddccbbaa99887766554433221100);
   cn = 2636; teststor_sivvv(v128_store8_lane, 4'he, 32'h0e, 128'hffffffffffffffffffffffffffffffff, 128'h00ee0000000000000000000000000000, 128'h00eeddccbbaa99887766554433221100);
   cn = 2637; teststor_sivvv(v128_store8_lane, 4'hf, 32'h0f, 128'hffffffffffffffffffffffffffffffff, 128'hff000000000000000000000000000000, 128'hffeeddccbbaa99887766554433221100);

   repeat (100) @(posedge slow_clock);
   $display(" Finish at cycle %5d after %5d testcases",count, cn);
   $finish();
   end

task  teststor_svvv; // 1 address, 2 vectors, 1 vector expected
input   [7:0]  func;
input  [31:0]  scalar;
input [127:0]  data0;
input [127:0]  data1;
input [127:0]  expect;
   @(posedge slow_clock); #2;
   debugpush(data0);
   debugpush(data1);
   pmosi       = {3'b11,func, 4'h0, scalar};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(posedge slow_clock); #2;
   @(negedge slow_clock); #1;
   if (mut.iVR.V0  !== expect ) $display(    "%d Error  Case# %5d No Pop -- expected %h found %h",count,cn,expect,mut.iVR.V0);
   @(posedge slow_clock); #4;

   if (WM[scalar[15:4]] !== data1) $display("%d Error  Case# %5d No Mem Update -- expected %h found %h",count,cn,data1,WM[aWM[15:4]]);
   @(posedge slow_clock); #4;
   @(posedge slow_clock); #2;
endtask

task  teststor_sivvv; // 1 address, 2 vectors, 1 vector expected
input   [7:0]  func;
input   [3:0]  imm;
input  [31:0]  scalar;
input [127:0]  data0;
input [127:0]  data1;
input [127:0]  expect;
   @(posedge slow_clock); #2;
   debugpush(data0);
   debugpush(data1);
   pmosi       = {3'b11,func, imm, scalar};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(posedge slow_clock); #2;
   @(negedge slow_clock); #1;
   if (mut.iVR.V0      !==    data1) $display("%d Error  Case# %5d expected %h found %h",count, cn, data1,  mut.iVR.V0);
   @(posedge slow_clock); #4;
   if (WM[scalar[15:4]] !== expect) $display("%d Error  Case# %5d expected %h found %h",count, cn, expect, WM[aWM[15:4]]);
   @(posedge slow_clock); #4;
   @(posedge slow_clock); #2;
endtask

task  testcase_vv;   // one vector in, one vector out
input   [7:0]  func;
input [127:0]  data;
input [127:0]  expect;
   @(posedge slow_clock); #2;
   debugpush(data);
   pmosi       = {3'b11,func, 4'h0, 32'h0};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(posedge slow_clock); #2;
   @(negedge slow_clock); #1;
   if (mut.iVR.nV0  !== expect) $display("%d Error  Case# %5d expected %h found %h",count,cn,expect,mut.iVR.nV0 );
   @(posedge slow_clock); #2;
endtask

task  testpair_vv8;   // one vector in, one vector out -- Emulate pairwise with a preshift
input   [7:0]  func;
input [127:0]  data;
input [127:0]  expect;
   @(posedge slow_clock); #2;
   debugpush(data);
   pmosi       = {3'b11,i16x8_shr_u, 4'h0, 32'h8};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   debugpush(data);
   @(posedge slow_clock); #4;
   pmosi       =  {3'b11,func, 4'h0, 32'h0};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(posedge slow_clock); #2;
   @(negedge slow_clock); #1;
   if (mut.iVR.nV0  !== expect) $display("%d Error  Case# %5d expected %h found %h",count,cn,expect,mut.iVR.nV0 );
   @(posedge slow_clock); #2;
endtask

task  testpair_vv16;   // one vector in, one vector out -- Emulate pairwise with a preshift
input   [7:0]  func;
input [127:0]  data;
input [127:0]  expect;
   @(posedge slow_clock); #2;
   debugpush(data);
   pmosi       = {3'b11,i32x4_shr_u, 4'h0, 32'h10};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   debugpush(data);
   @(posedge slow_clock); #4;
   pmosi       =  {3'b11,func, 4'h0, 32'h0};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(posedge slow_clock); #2;
   @(negedge slow_clock); #1;
   if (mut.iVR.nV0  !== expect) $display("%d Error  Case# %5d expected %h found %h",count,cn,expect,mut.iVR.nV0 );
   @(posedge slow_clock); #2;
endtask



task  testcase_v2v;   // one vector in, one vector out
input   [7:0]  func;
input [127:0]  data;
input [127:0]  expect0;
input [127:0]  expect1;
   @(posedge slow_clock); #2;
   debugpush(data);
   pmosi       = {3'b11,func, 4'h0, 32'h0};
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(posedge slow_clock); #2;
   @(negedge slow_clock); #1;
   if (mut.iVR.nV0 !== expect0) $display("%d Error  Case# %5d expect0 %h found %h",count,cn,expect0,mut.iVR.nV0 );
   if (mut.iVR.nV1 !== expect1) $display("%d Error  Case# %5d expect1 %h found %h",count,cn,expect1,mut.iVR.nV1);
   @(posedge slow_clock); #2;
endtask

task  testcase_xd8s;   // one vector in, one vector out
input   [7:0]  func;
input [127:0]  data;
input [127:0]  expect0;
input [127:0]  expect1;
   @(posedge slow_clock); #2;
   debugpush(data);
   debugpush(128'h0);
   @(posedge slow_clock); #4;
   pmosi       = {3'b11,i8x16_lt_s, 4'h0, 32'h0};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   @(posedge slow_clock); #4;
   debugpush(data);
   @(posedge slow_clock); #4;
   pmosi       = {3'b11,func, 4'h0, 32'h0};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(posedge slow_clock); #2;
   @(negedge slow_clock); #1;
   if (mut.iVR.nV0 !== expect0) $display("%d Error  Case# %5d expect0 %h found %h",count,cn,expect0,mut.iVR.nV0 );
   if (mut.iVR.nV1 !== expect1) $display("%d Error  Case# %5d expect1 %h found %h",count,cn,expect1,mut.iVR.nV1);
   @(posedge slow_clock); #2;
endtask


task  testcase_xd16s;   // one vector in, one vector out
input   [7:0]  func;
input [127:0]  data;
input [127:0]  expect0;
input [127:0]  expect1;
   @(posedge slow_clock); #2;
   debugpush(data);
   debugpush(128'h0);
   @(posedge slow_clock); #2;
   pmosi       = {3'b11,i16x8_lt_s, 4'h0, 32'h0};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   @(posedge slow_clock); #2;
   debugpush(data);
   @(posedge slow_clock); #2;
   pmosi       = {3'b11,func, 4'h0, 32'h0};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(posedge slow_clock); #2;
   @(negedge slow_clock); #1;
   if (mut.iVR.nV0 !== expect0) $display("%d Error  Case# %5d expect0 %h found %h",count,cn,expect0,mut.iVR.nV0);
   if (mut.iVR.nV1 !== expect1) $display("%d Error  Case# %5d expect1 %h found %h",count,cn,expect1,mut.iVR.nV1);
   @(posedge slow_clock); #2;
endtask

task  testcase_xd8u;   // one vector in, one vector out
input   [7:0]  func;
input [127:0]  data;
input [127:0]  expect0;
input [127:0]  expect1;
   @(posedge slow_clock); #2;
   debugpush(128'h0);
   debugpush(data);
   pmosi       = {3'b11,func, 4'h0, 32'h0};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(posedge slow_clock); #2;
   @(negedge slow_clock); #1;
   if (mut.iVR.nV0 !== expect0) $display("%d Error  Case# %5d expect0 %h found %h",count,cn,expect0,mut.iVR.nV0);
   if (mut.iVR.nV1 !== expect1) $display("%d Error  Case# %5d expect1 %h found %h",count,cn,expect1,mut.iVR.nV1);
   @(posedge slow_clock); #2;
endtask

task  testcase_xd16u;   // one vector in, one vector out
input   [7:0]  func;
input [127:0]  data;
input [127:0]  expect0;
input [127:0]  expect1;
   @(posedge slow_clock); #2;
   debugpush(128'h0);
   debugpush(data);
   pmosi       = {3'b11,func, 4'h0, 32'h0};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(posedge slow_clock); #2;
   @(negedge slow_clock); #1;
   if (mut.iVR.nV0 !== expect0) $display("%d Error  Case# %5d expect0 %h found %h",count,cn,expect0,mut.iVR.nV0);
   if (mut.iVR.nV1 !== expect1) $display("%d Error  Case# %5d expect1 %h found %h",count,cn,expect1,mut.iVR.nV1);
   @(posedge slow_clock); #2;
endtask


task  testcase_v2vxu;   // one vector in, one vector out
input   [7:0]  func;
input [127:0]  data;
input [127:0]  expect0;
input [127:0]  expect1;
   @(posedge slow_clock); #2;
   debugpush(data);
   pmosi       = {3'b11,func, 4'h0, 32'h0};
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(posedge slow_clock); #2;
   @(negedge slow_clock); #1;
   if (mut.iVR.nV0 !== expect0) $display("%d Error  Case# %5d expect0 %h found %h",count,cn,expect0,mut.iVR.nV0);
   if (mut.iVR.nV1 !== expect1) $display("%d Error  Case# %5d expect1 %h found %h",count,cn,expect1,mut.iVR.nV1);
   @(posedge slow_clock); #2;
endtask

task  testcase_vs;   // one vector in, one scalar out
input   [7:0]  func;
input [127:0]  data;
input  [31:0]  expect;
   @(posedge slow_clock); #2;
   debugpush(data);
   pmosi       = {3'b11,func, 4'h0, 32'h0};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(posedge slow_clock); #2;
   @(negedge slow_clock); #1;
   if (SIMDo !== expect) $display("%d Error  Case# %5d expected %h found %h",count,cn,expect,SIMDo);
   @(posedge slow_clock); #2;
endtask

task  testcase_sv;   // one scalar in, one vector out
input   [7:0]  func;
input  [31:0]  data;
input [127:0]  expect;
   @(posedge slow_clock); #2;
   debugpush(data);
   pmosi       = {3'b11,func, 4'h0, data};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(posedge slow_clock); #2;
   @(negedge slow_clock); #1;
   if (mut.iVR.nV0  !== expect) $display("%d Error  Case# %5d expected %h found %h",count,cn,expect,mut.iVR.nV0 );
   @(posedge slow_clock); #2;
endtask

task  testcase_vsv;   // one vector in, one scalar in, one vector out
input   [7:0]  func;
input [127:0]  data0;
input  [31:0]  data1;
input [127:0]  expect;
   @(posedge slow_clock); #2;
   debugpush(data0);
   pmosi       = {3'b11,func, 4'h0, data1};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(posedge slow_clock); #2;
   @(negedge slow_clock); #1;
   if (mut.iVR.nV0  !== expect) $display("%d Error  Case# %5d expected %h found %h",count,cn,expect,mut.iVR.nV0 );
   @(posedge slow_clock); #2;
endtask

task  testcase_vvv;   // two vectors in, one vector out
input   [7:0]  func;
input [127:0]  data0;
input [127:0]  data1;
input [127:0]  expect;
reg            flag;
   begin
   flag = 0;
   @(posedge slow_clock); #2;
   debugpush(data0);
   debugpush(data1);
   pmosi       = {3'b11,func, 4'h0, 32'h0};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(negedge slow_clock);
   flag = 1;
   if (mut.iVR.nV0 [127:0] !== expect[127:0]) $display("%d Error  Case# %5d expected %h found %h",count,cn,expect,mut.iVR.nV0 );
   @(posedge slow_clock); #2;
   flag = 0;
   end
endtask

task  testcase_dot;   // two vectors in, one vector out: dot multiply followed by i32x4 add
input   [7:0]  func;
input [127:0]  data0;
input [127:0]  data1;
input [127:0]  expect;
reg            flag;
   begin
   flag = 0;
   @(posedge slow_clock); #2;
   debugpush(data0);
   debugpush(data1);
   flag = 1;
   pmosi       = {3'b11,func,      4'h0, 32'h0};
   @(posedge slow_clock); #4;
   pmosi       = {3'b11,i32x4_add, 4'h0, 32'h0};
   while (!mut.SIMDir) @(negedge slow_clock);
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(negedge slow_clock);
   flag = 0;
   if (mut.iVR.nV0 [127:0] !== expect[127:0]) $display("%d Error  Case# %5d expected %h found %h",count,cn,expect,mut.iVR.nV0 );
   @(posedge slow_clock); #2;
   end
endtask

task  testcase_vvvv;   // three vectors in, one vector out
input   [7:0]  func;
input [127:0]  data0;
input [127:0]  data1;
input [127:0]  data2;
input [127:0]  expect;
reg            flag;
   flag = 0;
   @(posedge slow_clock); #2;
   debugpush(data0);
   debugpush(data1);
   debugpush(data2);
   pmosi       = {3'b11,func, 4'h0, 32'h0};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(negedge slow_clock); #2;
   flag = 1;
//   @(negedge slow_clock); #1;
   if (mut.iVR.nV0  !== expect) $display("%d Error  Case# %5d expected %h found %h",count,cn,expect,mut.iVR.nV0 );
   @(posedge slow_clock); #2;
   flag = 0;
endtask

task  testcase_vv2v;   //  two vectors in, two vectors out
input   [7:0]  func;
input [127:0]  data0;
input [127:0]  data1;
input [127:0]  expect0;
input [127:0]  expect1;
reg            flag;
   flag = 0;
   @(posedge slow_clock); #2;
   debugpush(data0);
   debugpush(data1);
   pmosi       = {3'b11,func, 4'h0, 32'h0};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(negedge slow_clock); #5;
   flag = 1;
   if (mut.iVR.nV0 !== expect0) $display($time,"%d Error  Case# %5d expected lower %h found %h",count,cn,expect0,mut.iVR.nV0);
   if (mut.iVR.nV1 !== expect1) $display($time,"%d Error  Case# %5d expected upper %h found %h",count,cn,expect1,mut.iVR.nV1);
   @(posedge slow_clock); #2;
   flag = 0;
endtask

task  testcase_sivv;   // scalar, immediate, vector in, one vector out
input   [7:0]  func;
input   [3:0]  imm;
input  [31:0]  scalar;
input [127:0]  data;
input [127:0]  expect;
reg            flag;
   flag = 0;
   @(posedge slow_clock); #2;
   debugpush(data);
   pmosi       = {3'b11,func, imm, scalar};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(negedge slow_clock); #2;
   flag = 1;
   @(posedge slow_clock); #2;
   @(posedge slow_clock); #2;
   if (mut.iVR.V0 !== expect) $display("%d Error  Case# %5d expected %h found %h",count,cn,expect,mut.iVR.V0);
   flag = 0;
endtask

task  testcase_ivs;   // immediate, vector in, one scalar out
input   [7:0]  func;
input   [3:0]  imm;
input [127:0]  data;
input  [31:0]  expect;
reg            flag;
   flag = 0;
   @(posedge slow_clock); #2;
   debugpush(data);
   pmosi       = {3'b11,func, imm, 32'h0};
   @(posedge slow_clock); #4;
   pmosi       = 47'h0;
   while (!mut.SIMDir) @(negedge slow_clock); #2;
   flag = 1;
//   @(negedge slow_clock); #1;
   if (SIMDo !== expect) $display("%d Error  Case# %5d expected %h found %h",count,cn,expect,SIMDo);
   @(posedge slow_clock); #2;
   flag = 0;
endtask

task  debugpush;
input [127:0] data;
reg   push;
   begin
   push  = 1'b1;
   @(posedge slow_clock);
   mut.iVR.V3 = mut.iVR.V2;
   mut.iVR.V2 = mut.iVR.V1;
   mut.iVR.V1 = mut.iVR.V0;
   mut.iVR.V0 = data;
   push  = 1'b0;
   @(posedge slow_clock);
   end
endtask

/*
reg [7:0] A,B,lo,hi;
reg [15:0]  sum, expect, co;
integer x,y,z;
initial begin
   z = 0;
   $display("\n\n Signed Addition Test \n\n"); // without sign-extension
   for (x=0;x<256;x=x+1)
      for (y=0;y<256;y=y+1) begin
         A[7:0] = x;
         B[7:0] = y;
         expect = $signed(A) + $signed(B);
        {co,lo} = {~A[7],A[6:0]} +  {~B[7],B[6:0]};
         sum    = {8{co}};
         $display(" %h + %h = %h expected %h", {{8{A[7]}},A[7:0]},{{8{B[7]}},B[7:0]},sum, expect);
         if (sum !== expect) $display(" Error  expected %h found %h",expect, sum);
         z = z+1;
         end
   $display("\n\n Unsigned Addition Test \n\n");
   for (x=0;x<256;x=x+1)
      for (y=0;y<256;y=y+1) begin
         A[7:0] = x;
         B[7:0] = y;
         expect = A + B;
         sum    = {8'h00,A[7],A[6:0]} +  {8'h00,B[7],B[6:0]};
         $display(" %4h + %4h = %4h expected %4h", A[7:0],B[7:0],sum, expect);
         if (sum !== expect) $display(" Error  expected %h found %h",expect, sum);
         z = z+1;
         end
   $display("\n\n %d Tests completed \n\n",z);
   end
*/
endmodule //t
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
`endif //verify_A2S_SIMD

