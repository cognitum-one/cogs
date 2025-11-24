//==============================================================================================================================
//
//   Copyright © 1996..2023 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name    : 4 port masked RAM
//                   : 1 high width high speed port
//                   : 1 high width port
//                   : 2 low  speed port of different widths!
//
//===============================================================================================================================
//      THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//
//    NOTES:
//
//    wRAM must run at the SALSA high-speed clock rate when SALSA is On.  This is too fast for direct RAM accesses!
//    So when Salsa is Off the clock switches down to the Tile clock rate and direct RAM accesses witll work.
//    SIMD needs a single cycle memory access to operate efficiently so the tile clock rate must be brought down to match
//
//===============================================================================================================================

`ifdef   RELATIVE_FILENAMES                    // DEFINE IN SIMULATOR COMMAND LINE
//   `include "A2_project_settings.vh"
//   `include "A2_defines.vh"
//   `include "COGNITUM_defines.vh"
`else
//   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src//include/A2_project_settings.vh"
//   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src//include/A2_defines.vh"
   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2Sv2r3_defines.vh"
`endif

module  wRAM #(
   parameter   DEPTH = 16384,                         // Memory size in bytes
   parameter   WIDTH = 32,
   parameter   BASE_A   = 32'h0,
   parameter   BASE_B   = 32'h0,
   parameter   BASE_W   = 32'h0,
   parameter   tACC  =  2,                            // Access time in clock cycles     -- supports up to 7 wait states
   parameter   tCYC  =  2                             // RAM cycle time in clock cycles
   )(
   output wire [`SMISO_WIDTH -1:0]  smiso,            // Salsa Port : 260 {      sack[3:0],        sbrd[255:0]}
   input  wire [`SMOSI_WIDTH -1:0]  smosi,            // Salsa Port : 271 {sreq,sen[3:0],swr, sadr[15:7], swrd[255:0]}

   output wire [`WMISO_WIDTH-1:0]   wmiso,            // SIMD  Port : 130 {WMg, WMk, WMo[127:0]}
   input  wire [`WMOSI_WIDTH-1:0]   wmosi,            // SIMD  Port : 290 {eWM, wWM, iWM[127:0], mWM[127:0], aWM[31:0]}

   output wire [`DMISO_WIDTH -1:0]  amiso,            // A2S   Port :  34 {agnt, aack,             ardd[31:0]}
   input  wire [`DMOSI_WIDTH -1:0]  amosi,            // A2S   Port :  66 {areq, awr,  awrd[31:0], aadr[31:0]}

   output wire [`DMISO_WIDTH+31:0]  bmiso,            // RWAY  Port :  66 {bgnt, back, brd0[31:0], brd1[31:0]}
   input  wire [`DMOSI_WIDTH -1:0]  bmosi,            // RWAY  Port :  66 {breq, bwr,  bwrd[31:0], badr[31:0]}

   input  wire                      bacpt,            // b-side acknowledge has been accepted

   input  wire                      SalsaOn,          // Salsa is active, all other accesses fail -- no acknowledge
   input  wire                      reset,
   input  wire                      fast_clock,
   input  wire                      tile_clock
   );

//localparam     LG2D  = `LG2(DEPTH);                   // e.g. 65536 -> 16
localparam     LG2D  = `LG2(DEPTH * WIDTH /8);        // e.g. 65536 -> 16 address bits

genvar         i;

// Clock domain to match the smosi/smiso ========================================================================================
wire  RAM_clock;

A2_ClockMux i_CM0 (
   .clocko  (RAM_clock),
   .clocka  (tile_clock),
   .clockb  (fast_clock),
   .select  (SalsaOn),
   //.hold    (1'b0), //JLPL
   //.reset   (reset) //JLPL
   .reseta  (reset), //JLPL
   .resetb  (reset) //JLPL
   );

// Decollate Salsa buses ========================================================================================================
wire  [255:0]  srdd;    // 32 bytes       S read data
wire  [255:0]  swrd;    // 32 bytes       S write data
wire  [ 15:7]  sadr;    // Upper address
wire  [  3:0]  sen;     // decoded from byte address bits[6:5] interleaved between word width and upper address
reg   [  3:0]  sack;
wire           sreq;
wire           swr;

assign   {sreq, sen[3:0], swr, sadr[15:7], swrd[255:0]}     =  smosi[270:0];   // 1+4+1+9+256 = 271

// Decollate SIMD buses =========================================================================================================
wire  [127:0]  wrdd;    // 16 bytes       W read data
wire  [127:0]  wwrd;    // 16 bytes       W write data
wire  [127:0]  wmsk;    // 16 bytes       W mask
wire  [ 31:0]  wadr;    // Upper address
wire           wack;
wire           wgnt;
wire           wreq;
wire           wwr;

assign   {wreq, wwr, wwrd[127:0], wmsk[127:0], wadr[31:0]}   =  wmosi[289:0];  // 1+1+128+128+32  = 290

wire  [  7:0]  wen = wreq << wadr[6:4];

// Decollate RAM buses ==========================================================================================================
wire  [ 63:0]  abrd;
wire  [ 31:0]  awrd, bwrd;
wire  [ 31:0]  aadr, badr;
wire           areq, breq;
wire           agnt, bgnt;
wire           awr,  bwr;
wire           aack, back;
wire           reqA, reqB, reqW, reqS;
wire           asel, bsel;
wire           reqM, mack;

wire dummya; //JLPL
wire dummyb; //JLPL

//assign   {areq, awr, awrd[31:0], aadr[31:0]} =  amosi[65:0],        // 1+1+32+32   =   66 //JLPL
//         {breq, bwr, bwrd[31:0], badr[31:0]} =  bmosi[65:0];        // 1+1+32+32   =   66 //JLPL
assign   {areq, awr, dummya, awrd[31:0], aadr[31:0]} =  amosi[66:0],        // 1+1+1+32+32   =   67 //JLPL - Update to 67
         {breq, bwr, dummyb, bwrd[31:0], badr[31:0]} =  bmosi[66:0];        // 1+1+1+32+32   =   67 //JLPL - Update to 67

// Generate requests
assign   reqA  =  ~SalsaOn & areq & (aadr[31:LG2D] == BASE_A[31:LG2D]),
         reqB  =  ~SalsaOn & breq & (badr[31:LG2D] == BASE_B[31:LG2D]),
         reqW  =  ~SalsaOn & wreq & (wadr[31:LG2D] == BASE_W[31:LG2D]),
         reqS  =   SalsaOn & sreq;

// FSM for RAM mode =============================================================================================================

reg   [  3:0]  SSM;                          // Selection State Machine
reg   [  7:0]  WSM;                          // Wait State Machine

localparam  [1:0] IDLE = 0, AACC = 1, BACC = 2, STAY = 3;
// A has absolute priority over B!!!
always @(posedge tile_clock)
   if (reset)                             SSM <= `tCQ 1'b1 << IDLE;
   else (* parallel_case *) case (1'b1)
      SSM[IDLE]   : if (reqA  && !reqW )  SSM <= `tCQ 1'b1 << AACC;
               else if (reqB  && !reqW )  SSM <= `tCQ 1'b1 << BACC;
      SSM[AACC]   : if (mack  &&  reqB )  SSM <= `tCQ 1'b1 << BACC;
               else if (mack  && !reqA )  SSM <= `tCQ 1'b1 << IDLE;
      SSM[BACC]   : if (mack  && !bacpt)  SSM <= `tCQ 1'b1 << STAY;
               else if (mack  &&  reqA )  SSM <= `tCQ 1'b1 << AACC;
               else if (mack  &&  reqB )  SSM <= `tCQ 1'b1 << BACC;
               else if (mack           )  SSM <= `tCQ 1'b1 << IDLE;
      SSM[STAY]   : if (          reqA )  SSM <= `tCQ 1'b1 << AACC;
               else if (bacpt &&  reqB )  SSM <= `tCQ 1'b1 << BACC;
               else if (bacpt          )  SSM <= `tCQ 1'b1 << IDLE;
      endcase

assign   agnt  =  mack |  SSM[IDLE],
         bgnt  = ~reqA &  agnt,
         wgnt  =  reqW &  SSM[IDLE],
         asel  =  reqA &  agnt,
         bsel  =  reqB &  bgnt,
         reqM  =  asel |  bsel;

// Multiplex between narrow RAM buses ===========================================================================================
wire           mwr   =  asel &  awr   |  bsel &  bwr;
wire  [ 7:0]   abnk  =  asel << aadr[6:4];
wire  [ 7:0]   bbnk  =  bsel << badr[6:4];
wire  [ 7:0]   men   =  abnk[7:0]     |  bbnk[7:0];
wire  [31:0]   madr;
wire  [31:0]   mwrd;

A2_mux #(2,32) iAWM  (.z(madr[31:0]), .s({asel,bsel}), .d({aadr[31:0],badr[31:0]}));
A2_mux #(2,32) iIWM  (.z(mwrd[31:0]), .s({asel,bsel}), .d({awrd[31:0],bwrd[31:0]}));

wire [255:0]   mmsk  =  32'hffffffff << (32 * madr[4:2]);

// RAM Blocks ===================================================================================================================
wire  [1023:0] WRo;
wire  [ 255:0] iWR, mWR;
wire  [  15:2] aWR;
wire  [   7:0] eWR;

A2_mux #(3,256) iIWR (.z(iWR[255:0]), .s({reqS,reqW,reqM}), .d({swrd[255:0], {2{wwrd[127:0]}}, {8{mwrd[31:0]}}}));

A2_mux #(3,256) iMWR (.z(mWR[255:0]), .s({reqS,reqW,reqM}), .d({{256{1'b1}}, {2{wmsk[127:0]}}, mmsk[255:0] }));

A2_mux #(3,14)  iAWR (.z(aWR[15:2]),  .s({reqS,reqW,reqM}), .d({sadr[15:7],5'h0, wadr[15:2],   madr[15:2]}));

A2_mux #(3,8)   iEWR (.z(eWR[7:0]),   .s({reqS,reqW,reqM}), .d({{2{sen[3]}},{2{sen[2]}},{2{sen[1]}},{2{sen[0]}},wen[7:0],men[7:0]}));

wire  wWR = reqS & swr | reqW & wwr | reqM & mwr;

wire tie_high = 1'b1; //JLPL
wire tie_low = 1'b0; //JLPL

wire  [7:0]  clock_RAM;

`ifdef REAL_DEVICE //JLPL

PREICG_X2N_A7P5PP84TR_C14 iCGxx0 (.ECK(clock_RAM[0]), .CK(RAM_clock), .E(eWR[0]), .SE(1'b0)); // RTT 20250727
PREICG_X2N_A7P5PP84TR_C14 iCGxx1 (.ECK(clock_RAM[1]), .CK(RAM_clock), .E(eWR[1]), .SE(1'b0));
PREICG_X2N_A7P5PP84TR_C14 iCGxx2 (.ECK(clock_RAM[2]), .CK(RAM_clock), .E(eWR[2]), .SE(1'b0));
PREICG_X2N_A7P5PP84TR_C14 iCGxx3 (.ECK(clock_RAM[3]), .CK(RAM_clock), .E(eWR[3]), .SE(1'b0));
PREICG_X2N_A7P5PP84TR_C14 iCGxx4 (.ECK(clock_RAM[4]), .CK(RAM_clock), .E(eWR[4]), .SE(1'b0));
PREICG_X2N_A7P5PP84TR_C14 iCGxx5 (.ECK(clock_RAM[5]), .CK(RAM_clock), .E(eWR[5]), .SE(1'b0));
PREICG_X2N_A7P5PP84TR_C14 iCGxx6 (.ECK(clock_RAM[6]), .CK(RAM_clock), .E(eWR[6]), .SE(1'b0));
PREICG_X2N_A7P5PP84TR_C14 iCGxx7 (.ECK(clock_RAM[7]), .CK(RAM_clock), .E(eWR[7]), .SE(1'b0));


   `ifdef POWER_PINS   //JLPL
rfsphs512X128WM2 i_WorkRAM0 (.VDDCE (tie_high), .VDDPE (tie_high), .VSSE  (tie_low), .Q(WRo[ 127:0  ]), .CLK(clock_RAM[0]), .WEN(~mWR[127:0]),  .A(aWR[15:7]), .D(iWR[127:0]),  .GWEN(~wWR), .CEN(~eWR[0]), .STOV(1'b0), .EMA(3'b010), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1)); //JLPL
rfsphs512X128WM2 i_WorkRAM1 (.VDDCE (tie_high), .VDDPE (tie_high), .VSSE  (tie_low), .Q(WRo[ 255:128]), .CLK(clock_RAM[1]), .WEN(~mWR[255:128]),.A(aWR[15:7]), .D(iWR[255:128]),.GWEN(~wWR), .CEN(~eWR[1]), .STOV(1'b0), .EMA(3'b010), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1)); //JLPL
rfsphs512X128WM2 i_WorkRAM2 (.VDDCE (tie_high), .VDDPE (tie_high), .VSSE  (tie_low), .Q(WRo[ 383:256]), .CLK(clock_RAM[2]), .WEN(~mWR[127:0]),  .A(aWR[15:7]), .D(iWR[127:0]),  .GWEN(~wWR), .CEN(~eWR[2]), .STOV(1'b0), .EMA(3'b010), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1)); //JLPL
rfsphs512X128WM2 i_WorkRAM3 (.VDDCE (tie_high), .VDDPE (tie_high), .VSSE  (tie_low), .Q(WRo[ 511:384]), .CLK(clock_RAM[3]), .WEN(~mWR[255:128]),.A(aWR[15:7]), .D(iWR[255:128]),.GWEN(~wWR), .CEN(~eWR[3]), .STOV(1'b0), .EMA(3'b010), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1)); //JLPL
rfsphs512X128WM2 i_WorkRAM4 (.VDDCE (tie_high), .VDDPE (tie_high), .VSSE  (tie_low), .Q(WRo[ 639:512]), .CLK(clock_RAM[4]), .WEN(~mWR[127:0]),  .A(aWR[15:7]), .D(iWR[127:0]),  .GWEN(~wWR), .CEN(~eWR[4]), .STOV(1'b0), .EMA(3'b010), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1)); //JLPL
rfsphs512X128WM2 i_WorkRAM5 (.VDDCE (tie_high), .VDDPE (tie_high), .VSSE  (tie_low), .Q(WRo[ 767:640]), .CLK(clock_RAM[5]), .WEN(~mWR[255:128]),.A(aWR[15:7]), .D(iWR[255:128]),.GWEN(~wWR), .CEN(~eWR[5]), .STOV(1'b0), .EMA(3'b010), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1)); //JLPL
rfsphs512X128WM2 i_WorkRAM6 (.VDDCE (tie_high), .VDDPE (tie_high), .VSSE  (tie_low), .Q(WRo[ 895:768]), .CLK(clock_RAM[6]), .WEN(~mWR[127:0]),  .A(aWR[15:7]), .D(iWR[127:0]),  .GWEN(~wWR), .CEN(~eWR[6]), .STOV(1'b0), .EMA(3'b010), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1)); //JLPL
rfsphs512X128WM2 i_WorkRAM7 (.VDDCE (tie_high), .VDDPE (tie_high), .VSSE  (tie_low), .Q(WRo[1023:896]), .CLK(clock_RAM[7]), .WEN(~mWR[255:128]),.A(aWR[15:7]), .D(iWR[255:128]),.GWEN(~wWR), .CEN(~eWR[7]), .STOV(1'b0), .EMA(3'b010), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1)); //JLPL
   `else //JLPL
rfsphs512X128WM2 i_WorkRAM0 (.Q(WRo[ 127:0  ]), .CLK(clock_RAM[0]), .WEN(~mWR[127:0]),  .A(aWR[15:7]), .D(iWR[127:0]),  .GWEN(~wWR), .CEN(~eWR[0]), .STOV(1'b0), .EMA(3'b010), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1)); //JLPL
rfsphs512X128WM2 i_WorkRAM1 (.Q(WRo[ 255:128]), .CLK(clock_RAM[1]), .WEN(~mWR[255:128]),.A(aWR[15:7]), .D(iWR[255:128]),.GWEN(~wWR), .CEN(~eWR[1]), .STOV(1'b0), .EMA(3'b010), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1)); //JLPL
rfsphs512X128WM2 i_WorkRAM2 (.Q(WRo[ 383:256]), .CLK(clock_RAM[2]), .WEN(~mWR[127:0]),  .A(aWR[15:7]), .D(iWR[127:0]),  .GWEN(~wWR), .CEN(~eWR[2]), .STOV(1'b0), .EMA(3'b010), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1)); //JLPL
rfsphs512X128WM2 i_WorkRAM3 (.Q(WRo[ 511:384]), .CLK(clock_RAM[3]), .WEN(~mWR[255:128]),.A(aWR[15:7]), .D(iWR[255:128]),.GWEN(~wWR), .CEN(~eWR[3]), .STOV(1'b0), .EMA(3'b010), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1)); //JLPL
rfsphs512X128WM2 i_WorkRAM4 (.Q(WRo[ 639:512]), .CLK(clock_RAM[4]), .WEN(~mWR[127:0]),  .A(aWR[15:7]), .D(iWR[127:0]),  .GWEN(~wWR), .CEN(~eWR[4]), .STOV(1'b0), .EMA(3'b010), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1)); //JLPL
rfsphs512X128WM2 i_WorkRAM5 (.Q(WRo[ 767:640]), .CLK(clock_RAM[5]), .WEN(~mWR[255:128]),.A(aWR[15:7]), .D(iWR[255:128]),.GWEN(~wWR), .CEN(~eWR[5]), .STOV(1'b0), .EMA(3'b010), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1)); //JLPL
rfsphs512X128WM2 i_WorkRAM6 (.Q(WRo[ 895:768]), .CLK(clock_RAM[6]), .WEN(~mWR[127:0]),  .A(aWR[15:7]), .D(iWR[127:0]),  .GWEN(~wWR), .CEN(~eWR[6]), .STOV(1'b0), .EMA(3'b010), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1)); //JLPL
rfsphs512X128WM2 i_WorkRAM7 (.Q(WRo[1023:896]), .CLK(clock_RAM[7]), .WEN(~mWR[255:128]),.A(aWR[15:7]), .D(iWR[255:128]),.GWEN(~wWR), .CEN(~eWR[7]), .STOV(1'b0), .EMA(3'b010), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1)); //JLPL
   `endif             //JLPL

`else
//
for (i=0; i<8; i=i+1) begin : WRs
   preicg1 iCGxx (                      // RTT 20250727
      .ECK  (clock_RAM[i]),
      .CK   (RAM_clock),
      .E    (eWR[i]),
      .SE   (1'b0)
      );
   mRAM #(
      .DEPTH (512),
      .WIDTH (128)
      ) i_RAM (
      .RAMo  ( WRo[128*i    +:128]),
      .iRAM  (iWR [128*(i%2)+:128]),   // Parentheses to ensure correct precedence! modulus is lower than multiply
      .mRAM  (mWR [128*(i%2)+:128]),
      .aRAM  (aWR [15:7]),
      .eRAM  (eWR [i]),
      .wRAM  (wWR),
      .clock (clock_RAM[i])
      );
   end
//
`endif //JLPL
// Capture Bank and lower addresses for narrower accesses -----------------------------------------------------------------------
reg   [7:0] Bank;
reg   [3:2] Ladr;
always @(posedge RAM_clock) if (|eWR[7:0])  Bank[7:0] <= `tFCQ eWR[7:0];
always @(posedge RAM_clock) if (|eWR[7:0])  Ladr[3:2] <= `tFCQ aWR[3:2];

// Staggered Outputs to Salsa ---------------------------------------------------------------------------------------------------

reg   [  3:0]  dly0, dly1, dly2, dly3;    // Delay enables for read select

always @(posedge RAM_clock) sack   <= `tFCQ {4{sreq}} & sen[3:0];

always @(posedge RAM_clock) if (|dly0[3:0] || sen[0] || reset) dly0[3:0] <= `tFCQ  sen[0] ? {3'b0,~swr} : {dly0[2:0],1'b0};
always @(posedge RAM_clock) if (|dly1[3:0] || sen[1] || reset) dly1[3:0] <= `tFCQ  sen[1] ? {3'b0,~swr} : {dly1[2:0],1'b0};
always @(posedge RAM_clock) if (|dly2[3:0] || sen[2] || reset) dly2[3:0] <= `tFCQ  sen[2] ? {3'b0,~swr} : {dly2[2:0],1'b0};
always @(posedge RAM_clock) if (|dly3[3:0] || sen[3] || reset) dly3[3:0] <= `tFCQ  sen[3] ? {3'b0,~swr} : {dly3[2:0],1'b0};

wire [3:0]  sWRX = {dly3[3], dly2[3], dly1[3], dly0[3]};

A2_mux #(4,256) iWRX (.z(srdd[255:0]), .s(sWRX[3:0]), .d(WRo[1023:0]));

// Output to SIMD Block --------------------------------------------------------------------------------------------------------

assign   wack        =  wreq;

A2_mux #(8,128) iWRS (.z(wrdd[127:0]), .s(Bank[7:0]), .d(WRo[1023:0]));

// Output to RAM Ports  --------------------------------------------------------------------------------------------------------

assign   abrd[63:0]  =  Ladr[3]  ? wrdd[127:64] : wrdd[63:0];

// Wait State Machine ----------------------------------------------------------------------------------------------------------
localparam  [2:0]  WAIT = 0,  LAST = 7;

wire  rreq  =  reqM & ~SalsaOn & ~wreq;

always @(posedge tile_clock)
   if (reset)                 WSM <= `tCQ 1'b1 << IDLE;
   else case(1'b1)
      WSM[WAIT] : if (rreq)   WSM <= `tCQ 1'b1 << (8 - tCYC);
      WSM[LAST] : if (rreq)   WSM <= `tCQ 1'b1 << (8 - tCYC);
                  else        WSM <= `tCQ 1'b1 << WAIT;
      default                 WSM <= `tCQ WSM  << 1'b1;
      endcase

assign   mack  =  WSM[8-tACC] |  SSM[WAIT],
         aack  =  mack        &  SSM[AACC],
         back  =  mack        &  SSM[BACC];

// Recollate buses for Coprocessor inputs ---------------------------------------------------------------------------------------
assign   smiso =  { sack[3:0],  srdd[255:0] },
         wmiso =  { wgnt, wack, wrdd[127:0] };

// Recollate buses for output ---------------------------------------------------------------------------------------------------
assign   amiso =  { agnt, aack, {32{aack}} & (Ladr[2] ? abrd[63:32] : abrd[31:00]) },
         bmiso =  { bgnt, back, {64{back}} &           {abrd[31:00],  abrd[63:32] }};

endmodule

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

`ifdef  verify_wRAM

module t;

parameter   SLOW_CLOCK_PERIOD  =  246;    // 4.065 GHz
parameter   FAST_CLOCK_PERIOD  =   82;    // 12.22 GHz
parameter   DEPTH              = 2048;
parameter   WIDTH              =  256;
parameter   BASE               =    0;
parameter   tACC               =   60;
parameter   tCYC               =   82;


wire [`SMISO_WIDTH-1:0]   smiso;         // Salsa Port        -- fast clock
wire [`NMISO_WIDTH-1:0]   nmiso;         // Neural Net Port   -- fast clock
wire [`DMISO_WIDTH-1:0]   tmiso;         // Tile Port         -- slow clock, pre-decoded bmosi
wire [`DMISO_WIDTH-1:0]   dmiso;         // A2S Data Port     -- slow clock

reg  [`SMOSI_WIDTH-1:0]   smosi;         // Salsa Port
reg  [`NMOSI_WIDTH-1:0]   nmosi;         // Neural Net Port
reg  [`DMOSI_WIDTH-1:0]   tmosi;         // Tile Port
reg  [`DMOSI_WIDTH-1:0]   dmosi;         // A2S Data Port
reg                       reset;
reg                       fast_clock;
reg                       slow_clock;
integer i;

initial begin
   slow_clock  =  1'b0;
   forever
      #(SLOW_CLOCK_PERIOD/2) slow_clock = ~slow_clock;
   end

initial begin
   fast_clock  =  1'b0;
   forever
      #(FAST_CLOCK_PERIOD/2) fast_clock = ~fast_clock;
   end

wRAM #(
   .DEPTH(DEPTH),             // CoP DEPTH = 2048
   .WIDTH(WIDTH),             // CoP WIDTH =  256
   .BASE (BASE ),             // Base Address
   .tACC (tACC ),             // Access time in clock cycles     -- supports up to 7 wait states
   .tCYC (tCYC )              // RAM cycle time in clock cycles
   ) mut (
   .smiso(smiso),             // Salsa Port : 260 {      sack[3:0],        sbrd[255:0]}
   .nmiso(nmiso),             // SIMD  Port : 260 {wgnt, wack[3:0],        wrdd[255:0]}
   .amiso(amiso),             // RAM   Port :  34 {agnt, aack,              ardd[31:0]}
   .bmiso(bmiso),             // RAM   Port :  66 {bgnt, back, brd0[31:0],  brd1[31:0]}

   .smosi(smosi),             // Salsa Port : 272 {sreq,sen[3:0],swr, sadr[9:0], swrd[255:0]}
   .nmosi(nmosi),             // SIMD  Port : 272 {wreq,wen[3:0],nwr, wadr[9:0], wwrd[255:0]}
   .amosi(amosi),             // RAM   Port :  50 {areq, awr,  awrd[31:0], aadr[17:2]}
   .bmosi(bmosi),             // RAM   Port :  50 {breq, bwr,  bwrd[31:0], badr[17:2]}

   .SalsaOn(SalsaOn),         // Salsa is active, all other accesses fail -- no acknowledge
   .reset(reset),
   .fast_clock(fast_clock),
   .tile_clock(tile_clock)
   );

reg   [31:0]   addr, data;

initial begin
   reset       =  1'b0;
   smosi       =  {`SMOSI_WIDTH{1'b0}};
   nmosi       =  {`NMOSI_WIDTH{1'b0}};
   dmosi       =  {`DMOSI_WIDTH{1'b0}};
   tmosi       =  {`DMOSI_WIDTH{1'b0}};
   #100;
   @(posedge slow_clock) `tCQ;
   reset       =  1'b1;
   repeat (10) @(posedge slow_clock) `tCQ;
   reset       =  1'b0;
   repeat (10) @(posedge slow_clock) `tCQ;
   // Write d ------------------------------------------------
   dmosi       =  {1'b1, 1'b1, 32'h600df1d0, 32'h8000_0100 };
   repeat (01) @(posedge slow_clock) `tCQ;
   while (!dmiso[33])  @(posedge slow_clock) `tCQ;
   dmosi       =  {1'b0, 1'b0, 32'hdeadbeef, 32'h0};
   repeat (01) @(posedge slow_clock) `tCQ;
   while (!dmiso[32])  @(posedge slow_clock) `tCQ;

   repeat (10) @(posedge slow_clock) `tCQ;
   // Read d -------------------------------------------------
   dmosi       =  {1'b1, 1'b0, 32'h00000000, 32'h8000_0100};
   repeat (01) @(posedge slow_clock) `tCQ;
   while (!dmiso[33])  @(posedge slow_clock) `tCQ;
   dmosi       =  {1'b0, 1'b0, 32'hdeadbeef, 32'h0};
   repeat (01) @(posedge slow_clock) `tCQ;
   while (!dmiso[32])  @(posedge slow_clock) `tCQ;

   repeat (10) @(posedge slow_clock) `tCQ;
   // Write e ------------------------------------------------
   tmosi       =  {1'b1, 1'b1, 32'h600df1d0, 32'h8000_0100};
   repeat (01) @(posedge slow_clock) `tCQ;
   while (!tmiso[65])  @(posedge slow_clock) `tCQ;
   tmosi       =  {1'b0, 1'b0, 32'hdeadbeef, 32'h0};
   repeat (01) @(posedge slow_clock) `tCQ;
   while (!tmiso[64])  @(posedge slow_clock) `tCQ;

   repeat (10) @(posedge slow_clock) `tCQ;
   // Read e -------------------------------------------------
   tmosi       =  {1'b1, 1'b0, 32'h00000000, 32'h8000_0100};
   repeat (01) @(posedge slow_clock) `tCQ;
   while (!tmiso[65])  @(posedge slow_clock) `tCQ;
   tmosi       =  {1'b0, 1'b0, 32'hdeadbeef, 32'h0};
   repeat (01) @(posedge slow_clock) `tCQ;
   while (!tmiso[64])  @(posedge slow_clock) `tCQ;

   repeat (10) @(posedge slow_clock) `tCQ;
   // Slow Arbitrate -----------------------------------------
   dmosi       =  {1'b1, 1'b1, 32'h600df1d0, 32'h8000_01ff};
   tmosi       =  {1'b1, 1'b1, 32'hfeedf00d, 32'h8000_0401};
   repeat (01) @(posedge slow_clock) `tCQ;
   while (!dmiso[33]) @(posedge slow_clock) `tCQ;
   dmosi       =  {1'b0, 1'b0, 32'hdeadbeef, 32'h0};
   repeat (01) @(posedge slow_clock) `tCQ;
   while (!dmiso[32])  @(posedge slow_clock) `tCQ;

   while (!tmiso[65]) @(posedge slow_clock) `tCQ;
   tmosi       =  {1'b0, 1'b0, 32'hdeadbeef, 32'h0};
   repeat (01) @(posedge slow_clock) `tCQ;
   while (!dmiso[64]) @(posedge slow_clock) `tCQ;

   repeat (20) @(posedge slow_clock) `tCQ;

   // Read to set priority the other way ---------------------
   dmosi       =  {1'b1, 1'b0, 32'h00000000, 32'h8000_0100};
   repeat (01) @(posedge slow_clock) `tCQ;
   while (!dmiso[33])  @(posedge slow_clock) `tCQ;
   dmosi       =  {1'b0, 1'b0, 32'hdeadbeef, 32'h0};
   repeat (01) @(posedge slow_clock) `tCQ;
   while (!dmiso[32])  @(posedge slow_clock) `tCQ;

   repeat (10) @(posedge slow_clock) `tCQ;
   // Slow Arbitrate the other way and read ------------------
   dmosi       =  {1'b1, 1'b0, 32'hdeadbeef, 32'h8000_01ff};
   tmosi       =  {1'b1, 1'b0, 32'hdeadbeef, 32'h8000_0401};
   repeat (01) @(posedge slow_clock) `tCQ;
   while (!dmiso[33] && !tmiso[65]) @(posedge slow_clock) `tCQ;
   if (dmiso[33]) dmosi =  {1'b0, 1'b0, 32'hb0015417, 32'h0};
   if (tmiso[65]) tmosi =  {1'b0, 1'b0, 32'hb0015417, 32'h0};
   repeat (01) @(posedge slow_clock) `tCQ;
   while (!dmiso[32] && !tmiso[64]) @(posedge slow_clock) `tCQ;
   while (!dmiso[33] && !tmiso[65]) @(posedge slow_clock) `tCQ;
   dmosi       =  {1'b0, 1'b0, 32'hdeadbeef, 32'h0};
   tmosi       =  {1'b0, 1'b0, 32'hdeadbeef, 32'h0};
   repeat (01) @(posedge slow_clock) `tCQ;
   while (!dmiso[32] && !tmiso[64]) @(posedge slow_clock) `tCQ;

   repeat (10) @(posedge slow_clock) `tCQ;
   $display(" Initial Test Done");
   repeat (10) @(posedge slow_clock) `tCQ;

   dmosi       =  {2'b00, 32'hdeadbeef, 32'h00000000 };
   tmosi       =  {2'b11, 32'hffffffff, 32'h80000000 };
   for (i=0; i<65536; i=i+4) begin
      // Write e ------------------------------------------------
      repeat (01)          @(posedge slow_clock); `tCQ;
      while (!tmiso[65])   @(posedge slow_clock); `tCQ;
      addr[31:0]  =  32'h80000000 | (i+4);
      data[31:0]  =  ~(i+4);
      tmosi       =  {1'b1, 1'b1, data, addr };
      repeat (01)          @(posedge slow_clock); `tCQ;
      while (!tmiso[64])   @(posedge slow_clock); `tCQ;
      end
   tmosi       =  {2'b00, 32'hdeadbeef, 32'h00000000 };
   $display(" RAM loaded from t-port");
   repeat (10) @(posedge slow_clock); `tCQ;

   for (i=0; i<65536; i=i+4) begin
      // Read d -------------------------------------------------
      addr[31:0]  =  32'h8000_0000 | i;
      dmosi       =  {2'b10, 32'hdeadbeef, addr };
      repeat (01)          @(posedge slow_clock);  `tCQ;
      while (!dmiso[33])   @(posedge slow_clock); `tCQ;
      dmosi       =  {2'b00, 32'hdeadbeef, 32'h00000000 };
      repeat (01)          @(posedge slow_clock); `tCQ;
      while (!dmiso[32])   @(posedge slow_clock); `tCQ;
      if (dmiso[31:0] !== ~i) $display("Error at %d: found %h",i,~dmiso[31:0]);
      end
   $display(" RAM read from d-port");
   repeat (10) @(posedge slow_clock) `tCQ;

   tmosi       =  {2'b00, 32'hdeadbeef, 32'h00000000 };
   dmosi       =  {2'b11, 32'hdeadbeef, 32'h00000000 };
   for (i=0; i<65536; i=i+4) begin
      // Write d -------------------------------------------------
      addr[31:0]  =  32'h8000_0000 | i;
      dmosi       =  {2'b11, i, addr};
      repeat (01)          @(posedge slow_clock); `tCQ;
      while (!dmiso[33])   @(posedge slow_clock); `tCQ;
      dmosi       =  {2'b00, 32'hdeadbeef, 32'h00000000};
      repeat (01)          @(posedge slow_clock); `tCQ;
      while (!dmiso[32])   @(posedge slow_clock); `tCQ;
      end
   dmosi       =  {2'b00, 32'h00000000, 32'h80000000};
   $display(" RAM written from d-port");
   repeat (10) @(posedge slow_clock); `tCQ;

   for (i=0; i<65536; i=i+8) begin
      // Read t -------------------------------------------------
      addr[31:0]  =  32'h8000_0000 | i;
      tmosi       =  {2'b10, 32'hdeadbeef, addr};
      repeat (01)          @(posedge slow_clock); `tCQ;
      while (!tmiso[65])   @(posedge slow_clock); `tCQ;
      tmosi       =  {2'b000, 32'hdeadbeef, 32'h00000000};
      repeat (01)          @(posedge slow_clock); `tCQ;
      while (!tmiso[64])   @(posedge slow_clock); `tCQ;
      data        = (i+4);
      if ((tmiso[63:32]==i) &~ (tmiso[31:0]== data)) $display("Error at %h: found %h & %h data %h",i,tmiso[31:0],tmiso[63:32],data);
      end
   $display(" RAM read from t-port");

      $display(" done");
   #1000;
   $finish;
   $stop;
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

