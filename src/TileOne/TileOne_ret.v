//===============================================================================================================================
//
//    Copyright © 2022, 2023 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to Authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//    Project Name         : Scrypt ASIC
//
//    Description          : A standalone single Scrypt Compute Tile
//                           A single Scrypt Engine comprising a microcontroller and a Sals20/8 Accelerator
//
//===============================================================================================================================
//    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//    Notes for Use and Synthesis
//
//    The only true Master in the system is TileZero.  However, we view the Raceway as the Master and the Tiles as Slaves.
//    Hence a Tile receives input over "xmosi" and responds over "xmiso"
//
//    Be careful when a Tile is accessing another tile as it will send it's request over "xmiso" and expect a response
//    over "xmosi"
//
//===============================================================================================================================

`include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh"
`include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/COGNITUM_defines.vh"
`include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2Sv2r3_defines.vh"
//`define  STANDALONE
//`define  FPGA_CLOCKS

module TileOne #(
   parameter   CODESIZE =  8192,                // Bytes
   parameter   DATASIZE =  8192,                // Bytes
   parameter   WORKSIZE =  65536                // Bytes  physically (512 x 1024-bits)
   )(
   // Output System Interconnect
   output wire                      xmiso_clk,  // 97       96     95:88     87:80     79:72     71:64     63:32      31:0
   output wire [`XMISO_WIDTH-1:0]   xmiso,      // xtmo,    xordy, xack[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xrd0[31:0],xrd1[31:0]}
   input  wire                      xmiso_fb,   //          xpop
   // Input System Interconnect
   input  wire                      xmosi_clk,  // 97       96     95:88     87:80     79:72     71:64     63:32      31:0
   input  wire [`XMOSI_WIDTH-1:0]   xmosi,      // xresetn, xpush, xcmd[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xwrd[31:0],xadr[31:0]}
   output wire                      xmosi_fb,   //          xirdy
`ifdef   STANDALONE
   // Passthroughs
   output wire [`XMISO_WIDTH-1:0]   DO1,     DO2,     DO3,     UO1,     UO2,     UO3,
   input  wire                      DO1_fb,  D02_fb,  DO3_fb,  UO1_fb,  UO2_fb,  UO3_fb,
   output wire                      DO1_ck,  D02_ck,  DO3_ck,  UO1_ck,  UO2_ck,  UO3_ck,

   input  wire [`XMISO_WIDTH-1:0]   DI1,     DI2,     DI3,     UI1,     UI2,     UI3,
   output wire                      DI1_fb,  DI2_fb,  DI3_fb,  UI1_fb,  UI2_fb,  UI3_fb,
   input  wire                      DI1_ck,  DI2_ck,  DI3_ck,  UI1_ck,  UI2_ck,  UI3_ck,
`endif   //STANDALONE
   // Global Interconnect
`ifdef   FPGA_CLOCKS
   input  wire                      fast_clock,
   input  wire                      tile_clock,
`endif   //FPGA_CLOCKS
   output wire                      flag_out,   // To   Tile-0
   input  wire                      flag_in,    // From Tile-0
   input  wire [             7:0]   myID        // Tile ID for use with 'src' and 'dst' fields
   );

`include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/COGNITUM_IO_addresses.vh"
`include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2Sv2r3_Registers.vh"

// Locals =======================================================================================================================
// bmosi\bmiso can access all memories and co-processors within a tile.  It is also used to send requests to other tiles from
// this tile's A2S.  bmosi\bmiso is considered lower priorty over on-tile resources and thus will stall pending another access.

localparam  [1:0] code = 2'b00, data = 2'b01, work = 2'b10, regs = 3;      // Upper 2 address bits [31:30]

// Physical Registers -----------------------------------------------------------------------------------------------------------
reg   [`XMOSI_WIDTH -3:0]  AMOSI, XMISO;
reg   [             31:0]  MTAGS, SCRATCH;
reg                        A2Sreset, A2Sreset_ack;
reg                        GW_LAST,  IO_LAST;

// Buses ========================================================================================================================
// External buses synch'd to tile clock
wire  [`XMOSI_WIDTH -3:0]  amosi;
wire  [`XMISO_WIDTH -3:0]  amiso_rs, amiso_rq, amiso;
// Memory buses
wire  [`DMOSI_WIDTH -1:0]  bmosi_cm, bmosi_dm, bmosi_wm, cmosi, dmosi;
wire  [`DMISO_WIDTH -1:0]  bmiso_cm, bmiso_dm,           cmiso, dmiso_dm, dmiso_wm, dmiso;
wire  [`DMISO_WIDTH+31:0]  bmiso_wm;               // Double with read-data
// Register I/O buses
wire  [`IMOSI_WIDTH -1:0]  imosi,    jmosi;
wire  [`IMOSI_WIDTH -1:0]  imosi_rg;
wire  [`IMISO_WIDTH -1:0]  imiso_gw, imiso_sh, imiso_ar, imiso_xs, imiso_lc, imiso;
`ifdef   A2S_ENABLE_SIMD
wire  [`WMISO_WIDTH-1:0]   wmiso;         // SIMD (wRAMport)miso 130 {WMg, WMk, WMo[127:0]}
wire  [`WMOSI_WIDTH-1:0]   wmosi;         // SIMD (wRAMport}mosi 290 {eWM, wWM, iWM[127:0], mWM[127:0], aWM[31:0]}
`endif
wire                       IOk;
// Salsa to WM buses
wire  [`SMOSI_WIDTH -1:0]  smosi;
wire  [`SMISO_WIDTH -1:0]  smiso;
// Nets -------------------------------------------------------------------------------------------------------------------------
// Clocks
wire                       ref_clock, tile_clock, fast_clock;
wire                       decode_CGD, decode_A2SRST;

wire                       wCG;
wire  [              7:0]  CGo;
// Synchronized resets
wire                       xinit, tinit, tile_reset;
// Decollated xmosi
wire  [             31:0]  xwdi, xadr;
wire  [              7:0]  xcmd, xtag, xdst, xsrc;
wire                       xclock, xresetn, xpush, xordy, xirdy;
// Incoming CDC
wire                       ld_amosi;
wire                       granted, accessing, broadcast;
// Decollated amosi
wire  [             31:0]  awdi,  aadr;
wire  [              7:0]  acmd,  atag,  adst,  asrc;
wire                       areq,  airdy, aordy, apush, apop;
// Register access from amosi
wire  [             31:0]  iRO;
wire  [             12:0]  aRO;
wire  [              1:0]  cRO;
wire                       eRE, qRG;
// Decollated imosi/jmosi
wire  [             31:0]  iIO,   iJO;
wire  [             12:0]  aIO,   aJO;
wire  [              1:0]  cIO,   cJO;
wire                       eIO,   eJO, eIE;
// Decollated & decoded bmosi
wire  [             31:0]  awrd;
wire                       bordy, bpop;
reg                        eCM,   eDM,   eWM,   eRG;
reg   [              1:0]  cRG;
wire                       valid_access;
wire                       bcast_tag_hit;
wire                       BMk, JOk, ERk, WMoff;
reg                        bwr, clr;
reg                        gpush;
wire                       EBk;
reg                        ROk;
reg                        tile_reset_req;
reg                        bcast_reset;
reg                        bcast_release;
wire                       sel_IO;

// A2S Processor
wire                       intpt_flag; //, intpt_mpp;
wire                       intpt;
wire  [              9:0]  cond;
wire                       A2Son;
// Tile local Registers
wire                       rc_SCR, rd_SCR, ld_SCR, sw_SCR, ld_MTG, rd_MTG, rd_CLKS, rd_DIVS;
wire  [             31:0]  SMo;
wire                       SMk;
// SHA256
wire                       intpt_sh_done;
// Xslas20/8I
wire                       intpt_xs_done;
wire                       SalsaOn;
// Gateway
wire                       intpt_gw_fini, intpt_gw_timo;
wire                       girdy, gordy, gpop;
// Tile Output
wire                       sel_gw;
wire  [             31:0]  brd0, brd1;
wire                       ld_xmiso;
// Debug
reg   [              7:0]  caseB;

// System Interconnect===========================================================================================================
// Accesses arriving at a tile are already decoded as to tile address (xdst==myID) and so may be used directly based on

// Decollate xmosi
//          97       96     95:88      87:80      79:72      71:64      63:32 31:0
assign     {xresetn, xpush, xcmd[7:0], xtag[7:0], xdst[7:0], xsrc[7:0], xwdi, xadr} = xmosi[`XMOSI_WIDTH-1:0];
assign      xmosi_fb =  xirdy;         // xpush and xirdy form FIFO-like controls
assign      xclock   =  xmosi_clk;

// Clock generator & divider ====================================================================================================
// Clocks and Dividers are setup by direct write to the control latches outside of the Tile CDCs.  They do NOT create responses
// Direct Write access to Tile Clock Control on the xmosi input side. No Clock Domain Crossing
assign       wCG  =   xpush & (xcmd==`SYSWR) & (xsrc==`TILEZ) & (xdst== myID ) & (xadr==aSYS_TILE_SET_CLOCKS);
// Reads are internal on jmosi/imiso

A2_ClockGen i_CG (
   .ref_clock  (ref_clock),
   // System Interconnect
   .iCG        (xwdi[3:0]),
   .wCG        (wCG),
   // Local Interconnect
   .rCG        ( rd_CLKS ),
   .CGo        ( CGo[3:0]),
   // Global
   .resetn     ( xresetn ),
   .clock      ( xclock  )
   );

A2_ClockDiv i_CD (
   // System Interconnect
   .iCD        (xwdi[7:4]),
   .wCD        (wCG),
   // Local Interconnect
   .rCD        (rd_CLKS),
   .CDo        ( CGo[7:4]),
   // Global
   .div_clock  (tile_clock),
   .db2_clock  (fast_clock),
   .ref_clock  (ref_clock ),
   .resetn     ( xresetn  ),
   .clock      ( xclock   )
   );

// Synchronize Resets ===========================================================================================================
// Reset from either side must ensure that all registers and synchronizers are correctly reset to a quiescent state.
// Resets also force input-ready and output-ready outputs to signal that the Tile is ready.
// Treat reset as asynchronous to both sides and ensure that the inactive edge is synchronized to both External and Tile clock
// active edges.

A2_reset u_rst_x (.q(xinit), .reset(~xresetn), .clock(xclock    ));   // Synchronised to External clock
A2_reset u_rst_i (.q(tinit), .reset(~xresetn), .clock(tile_clock));   // Synchronised to Tile clock

assign     tile_reset = tinit;

// Tile input Clock domain crossing =============================================================================================

A2_CDC i_CDCi  (                 //                                                             :        ^
   // Input Side -- from Raceway                                                 RACEWAY   +----:----+   |ld_amosi
   .wCDC    (xpush),             // INPUT  - Write          (FIFO push)            ------->|    :    |   |
   .CDCir   (xirdy),             // OUTPUT - Input ready    (FIFO not full)        <-------|    :    |   |
   .CDCld   (ld_amosi),          // OUTPUT - Load  input    (Load CDC register             |    :    |---+
   .ireset  (xinit),             // INPUT    reset synchronized to iclock          ------->|    :    |
   .iclock  (xclock),            // INPUT    clock                                 --------|>   :    |
   // Output Side -- to Tile                                                               |   CDC   |      TILE
   .rCDC    (apop ),             // INPUT    Read           (FIFO pop)                     |    :    |<------ apop
   .CDCor   (aordy),             // OUTPUT   Output Ready   (FIFO not empty)               |    :    |------> aordy
   .oreset  (tile_reset),        // INPUT    Reset synchronized to Tile clock              |    :    |<------
   .oclock  (tile_clock)         // INPUT    Tile clock                                    |    :   <|-------
   );                            //                                                        +----:----+
                                 //                                                             :
// Safe Capture of incoming request
always @(posedge xclock)   if (ld_amosi) AMOSI[`XMOSI_WIDTH-3:0] <= `tCQ xmosi[`XMOSI_WIDTH-3:0];

// Decollate amosi
assign            {acmd, atag, adst, asrc, awdi, aadr}  = AMOSI[`XMOSI_WIDTH-3:0];
assign    amosi = {acmd, atag, adst, asrc, awdi, aadr};

// Decode amosi to bmosi -- Incoming traffic from Raceway =======================================================================

// Incoming access to tile presents the 96-bit command-data-address word together with an output ready (bordy).  This stays
// stable in the tile clock domain until the request is popped from the CDC by the bpop signal.  The request is
// active until the module to be accessed grants the access.  This is close to a clock edge. So the pop is delayed into the
// next or later cycle when the acknowledge is created.  The request is inhibited, to prevent multiple accesses, and the
// acknowledge used to pop the request from the input queue CDC.

// Similarly the response is presented to the output CDC with the acknowledge signal as the push signal (bit-96) as long as the
// input ready signal (bmiso_fb) is true.   SO THE OUTGOING CDC's xmosi_fb (input_ready) MUST BE ACTIVE FOR ==BOTH== THE OUTGOING
// RESPONSE ==AND== THE POP OF THE INCOMING REQUEST SO PIPELINE BACKPRESSURE RULES ARE MET.

// ----------------------------- CHANGE FROM TILE PROTOTYPE --------------------------------
// As the DST field is not used for broadcast, it is used as the broadcast group control
// field that used to compare against the mtags
// -----------------------------------------------------------------------------------------
// Broadcast commands DO NOT send responses.
// bmosi destinations
//  request => CM, DM, WM, IO-SH, IO-XS, IO-GW,  ResetRelease.  response => IO-GW
//
// Handling illegal accesses:
// Illegal Requests, to non-existant memories etc send an error response
// Unsolicited Responses are ignored
//
//                         ordy      Destination match
assign   valid_access   =  aordy &  (adst==myID),              // Valid Incoming Request
         bcast_tag_hit  =  aordy &                             // BroadCast-Tag Hit
                        ( (adst ==  8'h00)                     // Always a full broadcast to every tile
                        | (adst ==  MTAGS[31:24])              // Broadcast to all tiles with a matching tag
                        | (adst ==  MTAGS[23:16])              // :
                        | (adst ==  MTAGS[15:08])              // :
                        | (adst ==  MTAGS[07:00])),            // :
         decode_CGD     = (aadr ==  aSYS_TILE_SET_CLOCKS)      // Not seen by A2S
                        | (aadr ==  aSYS_TILE_SET_DIVIDE),     // Not seen by A2S
         decode_A2SRST  = (aadr ==  aSYS_TILE_A2S_RESET)   & (adst==myID ) & (asrc==8'h00);

// Decode amosi to bmosi Accesses ===============================================================================================

localparam  grant = 33;
reg   bcast, spurious, unknown;

always @* begin
  {eCM,eDM,eWM,eRG} = 4'b0000;      // Tile Code, Tile Data, Tile Work, Tile Regs, Error (no-one addresses)
  {ROk,cRG[1:0],clr}= 4'b0000;      // Error, Clock load aknowledge, register command, clr as in read&clear
   bwr              = 1'b0;
   bcast            = 1'b0;
   tile_reset_req   = 1'b0;
   bcast_reset      = 1'b0;
   bcast_release    = 1'b0;
   gpush            = 1'b0;
   spurious         = 1'b0;         // Spurious response received
   unknown          = 1'b0;         // Unknown  request  received
   caseB            = 8'hff;
   if (aordy && airdy && !gordy && !accessing)  // incoming 'ordy' _AND_ outgoing 'irdy' _AND_ no new outgoing request
      casez ({aadr[31:30],acmd})
      //        reb cc num -- [request/ack, error, broadcast, command, number of words]
      //        ||| |  |                          __________ for read and clears
      //        ||| |  |                         /      ____ valid request for THIS tile
      // CODE   ||| |  |                        /      /
      {code,8'b10000001} : begin {eCM, bwr, clr, bcast} = {valid_access,  1'b1, 1'b1, 1'b0};   caseB =  0; end  // Read & Clear
      {code,8'b10001001} : begin {eCM, bwr, clr, bcast} = {valid_access,  1'b0, 1'b0, 1'b0};   caseB =  1; end  // Read
      {code,8'b1001?001} : begin {eCM, bwr, clr, bcast} = {valid_access,  1'b1, 1'b0, 1'b0};   caseB =  2; end  // Write \ Swap
      {code,8'b10110001} : begin {eCM, bwr, clr, bcast} = {bcast_tag_hit, 1'b1, 1'b0, 1'b1};   caseB =  3; end  // BroadCast write if Tag Hit (bcth)
      // DATA
      {data,8'b10000001} : begin {eDM, bwr, clr, bcast} = {valid_access,  1'b1, 1'b1, 1'b0};   caseB =  4; end  // Read & Clear
      {data,8'b10001001} : begin {eDM, bwr, clr, bcast} = {valid_access,  1'b0, 1'b0, 1'b0};   caseB =  5; end  // Read
      {data,8'b1001?001} : begin {eDM, bwr, clr, bcast} = {valid_access,  1'b1, 1'b0, 1'b0};   caseB =  6; end  // Write \ Swap
      {data,8'b10110001} : begin {eDM, bwr, clr, bcast} = {bcast_tag_hit, 1'b1, 1'b0, 1'b1};   caseB =  7; end  // BroadCast write if Tag Hit (bcth)
      // WORK
      {work,8'b10000001} : begin {eWM, bwr, clr, bcast} = {valid_access,  1'b1, 1'b1, 1'b0};   caseB =  8; end  // Read & Clear
      {work,8'b10001001} : begin {eWM, bwr, clr, bcast} = {valid_access,  1'b0, 1'b0, 1'b0};   caseB =  9; end  // Read 1
      {work,8'b10001010} : begin {eWM, bwr, clr, bcast} = {valid_access,  1'b0, 1'b0, 1'b0};   caseB = 10; end  // Read 2
      {work,8'b10010001} : begin {eWM, bwr, clr, bcast} = {valid_access,  1'b1, 1'b0, 1'b0};   caseB = 11; end  // Write \ Swap
      {work,8'b10011001} : begin {eWM, bwr, clr, bcast} = {valid_access,  1'b1, 1'b0, 1'b0};   caseB = 12; end  // Write \ Swap
      {work,8'b10110001} : begin {eWM, bwr, clr, bcast} = {bcast_tag_hit, 1'b1, 1'b0, 1'b1};   caseB = 13; end  // BroadCast write if Tag Hit (bcth)
      // REGS
      {regs,8'b10000001} : begin {eRG, cRG, clr, bcast} = {valid_access,  2'b00,1'b1, 1'b0};   caseB = 14; end  // Read & Clear
      {regs,8'b10001001} : begin {eRG, cRG, clr, bcast} = {valid_access,  2'b01,1'b0, 1'b0};   caseB = 15; end  // Read
      {regs,8'b1001?001} : begin {eRG, cRG, clr, bcast} = {valid_access,  2'b10,1'b0, 1'b0};   caseB = 16; end  // Write \ Swap
      {regs,8'b10110001} : begin {eRG, cRG, clr, bcast} = {bcast_tag_hit, 2'b11,1'b0, 1'b1};   caseB = 17; end  // Broadcast write if Tag Hit (bcth)
      // SYS B'CAST set/release
      {regs,8'b10100000} : begin {bcast_release, bcast} = {bcast_tag_hit,             1'b1};   caseB = 20; end  // System write to Release Reset -- No response
      {regs,8'b10111000} : begin {bcast_reset,   bcast} = {bcast_tag_hit,             1'b1};   caseB = 21; end  // System write to Set     Reset -- No Response
      {regs,8'b10010000} : begin {ROk,           bcast} = {valid_access,              1'b0};   caseB = 22; end  // System write to Clocks
      // RESP
      {2'b??,8'b0???????}: if (girdy) begin      gpush  =  valid_access;                       caseB = 23; end  // Incoming Response Back to Gateway
                           else       begin   spurious  =  1'b1;                               caseB = 24; end  // Spurious response
      default              begin              unknown   =  1'b1;                               caseB = 25; end  // ERROR
      endcase
   end

assign   awrd     =  clr ? 32'h0 :  awdi;
assign   areq     =  eCM |  eDM | eWM | eRG | bcast_release | bcast_reset | gpush;   // Summat's 'appening!
assign   qRG      =  eRG & (aadr[29:15] == 15'h0000);                                // Qualified enable Register access
//                 enable write data  address
//                          65   64    63:32 31:0
assign   bmosi_cm =  eCM ? {1'b1, bwr,  awrd, aadr} : 66'h0,          // To Code Memory
         bmosi_dm =  eDM ? {1'b1, bwr,  awrd, aadr} : 66'h0,          // To Data Memory
         bmosi_wm =  eWM ? {1'b1, bwr,  awrd, aadr} : 66'h0;          // To Work Memory
//                          47    46:45 44:13 12:0
assign   imosi_rg =  qRG ? {eRG, cRG,  awrd, aadr[14:2]} : 48'h0;     // To I/O (jmosi)

assign   granted  =  eCM &  bmiso_cm[33]
                  |  eDM &  bmiso_dm[33]
                  |  eWM &  bmiso_wm[65]
                  |  eRG & ~sel_IO
                  |  bcast_release | bcast_reset | decode_A2SRST
                  |  EBk |  gpush;

reg         [1:0] TSM;
localparam  [1:0] IDLE = 2'd0, ACCESS = 2'd1, BCAST_ACC = 2'd2, WAIT = 2'd3;

wire  taken =  bordy | girdy & gpush;

always @(posedge tile_clock)
   if (tile_reset)                                 TSM <= `tCQ IDLE;
   else (* parallel_case *) case(TSM)
      IDLE:       if (areq  && !granted          ) TSM <= `tCQ WAIT;
             else if (areq  &&  granted &&  bcast) TSM <= `tCQ BCAST_ACC;
             else if (areq  &&  granted && !taken) TSM <= `tCQ ACCESS;
      ACCESS:     if (bordy ||  girdy   &&  gpush) TSM <= `tCQ IDLE;
      BCAST_ACC:                                   TSM <= `tCQ IDLE;
      WAIT:       if (granted                    ) TSM <= `tCQ ACCESS;
      endcase

assign   accessing =  TSM == ACCESS,
         broadcast =  TSM == BCAST_ACC,
         apop      =  broadcast | bpop & bordy | gpush & girdy;                      // pop request or response from input

reg  [4:0]  timeout;
always @(posedge tile_clock)
   if (tile_reset || !accessing)    timeout <= `tCQ   8'h0;
   else if (accessing)              timeout <= `tCQ   timeout + 1'b1;

assign   EBk   = timeout[4] | spurious | unknown;

// A2S RESET CONTROL ------------------------------------------------------------------------------------------------------------
wire enA2SRST = eRG & decode_A2SRST;

always @(posedge tile_clock)
   if       ( tile_reset   ) A2Sreset  <= `tCQ 1'b1;
   else if  ( bcast_reset  ) A2Sreset  <= `tCQ 1'b1;
   else if  ( bcast_release) A2Sreset  <= `tCQ 1'b0;
   else if  ( enA2SRST     ) A2Sreset  <= `tCQ awrd[0];

//                    IOk          IOo
assign   imiso_ar =  {enA2SRST,{32{enA2SRST & A2Sreset}}};

// Multiplex A2S-I/O with incoming accesses via bmosi_rg -- A2S gets priority! ==================================================

assign   eIE               =  eIO    &  aIO[12];                        // Limit to IO outside of A2S

assign  {eIO,cIO,iIO,aIO}  =  imosi,                                    // Decollate imosi from A2S
        {eRE,cRO,iRO,aRO}  =  imosi_rg;                                 // Decollate imosi_rg

reg   [1:0] iFSM;
localparam  [1:0] EASY = 2'b00, HLDI = 2'b01, HLDJ = 2'b10;
//  Little 2-way arbiter
always @(posedge tile_clock)
   if (tile_reset)                        iFSM <= `tCQ  EASY;
   else case (iFSM)
      HLDI :  if ( IOk                )   iFSM <= `tCQ  EASY;           // Go back to IDLE on acknowledge
      HLDJ :  if ( JOk                )   iFSM <= `tCQ  EASY;           // Go back to IDLE on acknowledge
      default if ( eIE && !IOk        )   iFSM <= `tCQ  HLDI;           // EASY (IDLE) and new A2S access
         else if (!eIE &&  eRE && !JOk)   iFSM <= `tCQ  HLDJ;           // EASY (IDLE) and no A2S and incoming access
      endcase

assign   sel_IO      =  (iFSM == EASY) &  eIE
                     |  (iFSM == HLDI);

assign   eJO         =  sel_IO   ?  eIE        :  eRE,                  // Enable either A2S or bmosi
         cJO[ 1:0]   =  sel_IO   ?  cIO[1:0]   :  cRO,                  // A2S has priority Command
         iJO[31:0]   =  sel_IO   ?  iIO        :  iRO,                  // Write Data
         aJO[12:0]   =  sel_IO   ?  aIO        :  aRO;                  // Address

assign   jmosi       =  {eJO, cJO, iJO, aJO};

// Code Memory ==================================================================================================================

RAM_2P #(
   .DEPTH   (CODESIZE/4),
   .WIDTH   (`A2S_MACHINE_WIDTH),
   .TYPE    (code),
   .tACC    (`CM_NUM_CLOCKS_READ),
   .tCYC    (`CM_NUM_CLOCKS_CYCLE)
   ) i_CM   (
   .amiso   (cmiso),                   // A2S Code
   .amosi   (cmosi),
   .bmiso   (bmiso_cm),                // External
   .bmosi   (bmosi_cm),
   .bacpt   (bpop),                    // acknowledge accepted into XMISO
   .reset   (tile_reset),
   .clock   (tile_clock)
   );

// Data Memory ==================================================================================================================

RAM_2P  #(
   .DEPTH   (DATASIZE/4),
   .WIDTH   (`A2S_MACHINE_WIDTH),
   .TYPE    (data),
   .tACC    (`DM_NUM_CLOCKS_READ),
   .tCYC    (`DM_NUM_CLOCKS_CYCLE)
   ) i_DM   (
   .amiso   (dmiso_dm),                // A2S Data
   .amosi   (dmosi),
   .bmiso   (bmiso_dm),                // External
   .bmosi   (bmosi_dm),
   .bacpt   (bpop),                    // acknowledge accepted into XMISO
   .reset   (tile_reset),
   .clock   (tile_clock)
   );

// Work Memory ==================================================================================================================

wRAM  #(
   .DEPTH      (WORKSIZE/4),           // CoP DEPTH = 2048
   .WIDTH      (`A2S_MACHINE_WIDTH),   // CoP WIDTH =  256
   .TYPE       (work),                 // Base Address
   .tACC       (`WM_NUM_CLOCKS_READ),  // Access time in clock cycles     -- supports up to 7 wait states
   .tCYC       (`WM_NUM_CLOCKS_CYCLE)  // RAM cycle time in clock cycles
   ) i_WM      (
   .smiso      (smiso),                // Salsa Port : 260 {      sack[3:0],        sbrd[255:0]}
   .amiso      (dmiso_wm),             // RAM   Port :  34 {agnt, aack,              ardd[31:0]}
   .bmiso      (bmiso_wm),             // RAM   Port :  66 {bgnt, back, brd0[31:0],  brd1[31:0]}
   .smosi      (smosi),                // Salsa Port : 271 {sreq,sen[3:0],swr, sadr[15:7], swrd[255:0]}
`ifdef   A2S_ENABLE_SIMD
   .wmosi      (wmosi),                // SIMD  Port : 290={eWM, wWM, iWM[127:0], mWM[127:0], aWM[31:0]}
   .wmiso      (wmiso),                // SIMD  port : 130={WMg, WMk, WMo[127:0]}
`else
   .wmosi      (290'b0),
   .wmiso      (),
`endif
   .amosi      (dmosi),                // RAM   Port :  50 {areq, awr,  awrd[31:0], aadr[31:0]}
   .bmosi      (bmosi_wm),             // RAM   Port :  50 {breq, bwr,  bwrd[31:0], badr[31:0]}
   .bacpt      (bpop),                 // acknowledge accepted into XMISO

   .SalsaOn    (SalsaOn),              // Salsa is active, all other accesses fail -- no acknowledge
   .reset      (tile_reset),
   .fast_clock (fast_clock),
   .tile_clock (tile_clock)
   );

// Send results from memories ===================================================================================================

assign   dmiso =  dmiso_dm | dmiso_wm;

// A2Sv2 Processor ==============================================================================================================

A2_sync  u_cdc_f (.q(intpt_flag), .d(flag_in), .reset(tinit), .clock(tile_clock));   // Synchronise flag to Tile clock

assign   intpt       = |{intpt_sh_done, intpt_xs_done, intpt_gw_fini, intpt_gw_timo, intpt_flag};

assign   cond[9:0]   =   {1'b0,/*MPPcond[3:0],*/intpt_sh_done, intpt_xs_done, intpt_gw_fini, intpt_gw_timo, intpt_flag};

/*
A2Sv2r3 #(
   .WA2S(`A2S_MACHINE_WIDTH),
   .WINS(`A2S_INSTRUCTION_WIDTH),
   .DRSK(32),                             // A2S Depth of Return Stack
   .DDSK(32)                              // A2S Depth of Data Stack
   ) i_A2S (
   .dmosi(dmosi),                         // Data Memory   Master-out slave-in
   .dmiso(dmiso),                         // Data Memory   Master-in slave-out
   .cmosi(cmosi),                         // Code Memory   Master-out slave-in
   .cmiso(cmiso),                         // Code Memory   Master-in slave-out
   .imosi(imosi),                         // I/O  Register Master-out slave-in
   .imiso(imiso),                         // I/O  Register Master-in slave-out
`ifdef   A2S_ENABLE_SIMD
   .wmosi(wmosi),                         // SIMD Master-out slave-in
   .wmiso(wmiso),                         // SIMD Master-in slave-out
`endif
   .intpt(intpt),
   .ccode(cond),
   .activ(A2Son),
   .reset(A2Sreset),
   .fstck(fast_clock),
   .clock(tile_clock)
   );
*/
A2Sv2r3 i_A2S (
   .dmosi(dmosi),                         // Data Memory   Master-out slave-in
   .dmiso(dmiso),                         // Data Memory   Master-in slave-out
   .cmosi(cmosi),                         // Code Memory   Master-out slave-in
   .cmiso(cmiso),                         // Code Memory   Master-in slave-out
   .imosi(imosi),                         // I/O  Register Master-out slave-in
   .imiso(imiso),                         // I/O  Register Master-in slave-out
`ifdef   A2S_ENABLE_SIMD
   .wmosi(wmosi),                         // SIMD Master-out slave-in
   .wmiso(wmiso),                         // SIMD Master-in slave-out
`endif
   .intpt(intpt),
   .ccode(cond),
   .activ(A2Son),
   .reset(A2Sreset),
   .fstck(fast_clock),
   .clock(tile_clock)
   );

//// OLD
//A2Sv2r3 #(
//   .WA2S    (`A2S_MACHINE_WIDTH),
//   .WINS    (`A2S_INSTRUCTION_WIDTH),
//   .DRSK    (32),                                              // A2S Depth of Return Stack
//   .DDSK    (32)                                               // A2S Depth of Data Stack
//   ) i_A2S (
//   .cmosi   (cmosi),                                           // Code Memory  Master-out slave-in
//   .cmiso   (cmiso),                                           // Code Memory  Master-in slave-out
//   .dmosi   (dmosi),                                           // Data Memory  Master-out slave-in
//   .dmiso   (dmiso),                                           // Data Memory  Master-in slave-out
//   .imosi   (imosi),                                           // I/O Register Master-out slave-in
//
//`ifdef   A2S_ENABLE_FLOATING_POINT  //-------------------------------------------------------------------------------------------
//   `ifdef   A2S_ENABLE_SIMD
//   .imiso   ({IOk,imiso[31:0]} | imiso_fp | imiso_sd ),        // I/O Register Master-in slave-out with both FPU and SIMD
//   `else
//   .imiso   ({IOk,imiso[31:0]} | imiso_fp ),                   // I/O Register Master-in slave-out with just FPU
//   `endif
//`else
//   `ifdef   A2S_ENABLE_SIMD
//   .imiso   ({IOk,imiso[31:0]} | imiso_sd ),                   // I/O Register Master-in slave-out with just SIMD
//   `else
//   .imiso   ({IOk,imiso[31:0]}),                               // I/O Register Master-in slave-out without FPU or SIMD
//   `endif //---------------------------------------------------------------------------------------------------------------------
//`endif //A2S_ENABLE_FLOATING_POINT
//
//`ifdef   A2S_ENABLE_FLOATING_POINT
//   .fmosi   (fmosi),                                           // FPU Master(A2S)-out slave(FPU)-in
//   .fmiso   (fmiso),                                           // FPU Master(A2S)-in  slave(FPU)-out
//`endif
//`ifdef  A2S_ENABLE_SIMD
//   .pmosi   (pmosi),                                           // SIMD Master(A2S)-out slave(SIMD)-in
//   .pmiso   (pmiso),                                           // SIMD Master(A2S)-in  slave(SIMD)-out
//`endif //A2S_ENABLE_SIMD
//
//   .intpt   (intpt),
//   .ccode   (cond),
//   .activ   (A2Son),
//   .reset   (A2Sreset),
//   .clock   (tile_clock)
//   );
//
//`ifdef   A2S_ENABLE_FLOATING_POINT
//// Floating Point
//A2Sv2r3_SPFPU #(SPFPU_base,2) iFPU (
//   .imosi   (imosi),
//   .imiso   (imiso_fp),
//   .fmosi   (fmosi),
//   .fmiso   (fmiso),
//   .reset   (A2Sreset),
//   .clock   (tile_clock)
//   );
//`endif //A2S_ENABLE_FLOATING_POINT
//
//`ifdef  A2S_ENABLE_SIMD
//A2Sv2r3_SIMD #(SIMD_base,6) iSIMD (
//   .imosi      (imosi),                   // Slave data in         48 {cIO[2:0] aIO[12:0], iIO[31:0]}
//   .imiso      (imiso_sd),                // Slave data out        33 {IOk, IOo[31:0]}
//   .pmiso      (pmiso),                   // SIMD Port to A2S      33 { SIMDit,SIMDir,     SIMDo[31:0]}
//   .pmosi      (pmosi),                   // SIMD Port to A2S      46 {eSIMD, cSIMD[9:0], iSIMD [31:0]}
//   .wmosi      (wmosi),                   // SIMD Port to WRAM    290 {eWM, wWM, iWM[127:0], mWM[127:0], aWM[31:0]}
//   .wmiso      (wmiso),                   // SIMD port to WRAM    130 {WMg, WMk, WMo[127:0]}
//   .reset      (A2Sreset),
//   .fast_clock (fast_clock),
//   .clock      (tile_clock)
//   );
//`endif //A2S_ENABLE_SIMD

// I/O (Co-processors) ==========================================================================================================
// Tile Registers ---------------------------------------------------------------------------------------------------------------
localparam  LG2REGS  =  `LG2(TILE_range);
wire                    in_range =  aJO[12:LG2REGS] == TILE_base[12:LG2REGS];
wire  [TILE_range-1:0]  decode   =  in_range <<  aJO[LG2REGS-1:0];

assign   rc_SCR   =  eJO & (cJO==2'b00) & (decode[TILE_SCRATCH]),
         rd_SCR   =  eJO &                (decode[TILE_SCRATCH]),
         ld_SCR   =  eJO & (cJO==2'b10) & (decode[TILE_SCRATCH]),
         sw_SCR   =  eJO & (cJO==2'b11) & (decode[TILE_SCRATCH]),
         ld_MTG   =  eJO & (cJO==2'b10) & (decode[TILE_MATCH_TAGS]),
         rd_MTG   =  eJO & (cJO==2'b01) & (decode[TILE_MATCH_TAGS]),
         rd_CLKS  =  eJO & (cJO==3'b01) & (decode[TILE_RD_CLOCKS]),
         rd_DIVS  =  eJO & (cJO==3'b01) & (decode[TILE_RD_DIVIDE]);

always @(posedge tile_clock) if ( ld_SCR || sw_SCR ) SCRATCH <= `tCQ  iIO[31:0];
                        else if ( rc_SCR           ) SCRATCH <= `tCQ      32'h0;

always @(posedge tile_clock) if ( tile_reset       ) MTAGS   <= `tCQ      32'h0;
                        else if ( ld_MTG           ) MTAGS   <= `tCQ  iIO[31:0];

A2_mux #(3,32) iSMO (.z(SMo[31:0]), .s({rd_SCR,rd_MTG,rd_CLKS}), .d({SCRATCH, MTAGS, 24'h0,CGo[7:0]}));

assign   SMk         =  rd_SCR | rd_MTG | ld_MTG | rd_CLKS | rd_DIVS;

assign   imiso_lc    = {SMk,SMo};

// SHA256 -----------------------------------------------------------------------------------------------------------------------

A2_sha256_CoP #(SHA256_base) i_shaCoP (
   .imiso      (imiso_sh),                //
   .imosi      (jmosi),                   //
   .ok         (intpt_sh_done),           // Interrupt : Algorithm done
   .reset      (tile_reset),
   .clock      (tile_clock)
   );

// Xsalsa20/8I ------------------------------------------------------------------------------------------------------------------

A2_Xsalsa20_8I_CoP i_Salsa (
//A2_Xsalsa20_8I_CoP #(XSALSA_base,XSALSA_range) i_Salsa (
   .imiso      (imiso_xs),                //
   .imosi      (jmosi),                   //
   .done       (intpt_xs_done),           // Interrupt : Algorithm done
   // SRAM Interface
   .smosi      (smosi),                   // Salsa Master output to  SRAM slave input  (WM: Working memory)
   .smiso      (smiso),                   // Salsa Master input from SRAM slave output (WM: Working memory)
   // Global
   .SalsaOn    (SalsaOn),
   .slow_clock (tile_clock),
   .reset      (tile_reset),              // It takes 10 cycles to initalize from assertion of reset before first 'load' !!!
   .fast_clock (fast_clock)
   );

// SIMD -------------------------------------------------------------------------------------------------------------------------

//A2_SIMD_CoP #(`aSIMD_base) i_SIMD (
//   // IO Interface
//   .imiso      (imiso_mp),
//   .imosi      (jmosi),
//   .interrupt  (intpt_mpp),
//   // WRAM Interface
//   .nmosi      (nmosi),
//   .nmiso      (nmiso),
//   .nmask      (nmask),
//   // Conditions
//   .cond       (MPPcond[3:0]),            // AI block signaling conditions
//   // Global
//   .MPPon      (MPPon),
//   .clock      (tile_clock),
//   .reset      (tile_reset)
//   );

// Gateway ----------------------------------------------------------------------------------------------------------------------

A2_gateway_CoP #(GATEWAY_base,GATEWAY_range) i_GW (
   // Request Out
   .miso       (amiso_rq),       // ---->
   .ordy       (gordy),          // --->\/
   .pop        (gpop),           // <---/\
   // Response In
   .mosi       (amosi),          // <----
   .push       (gpush),          // <---\/
   .irdy       (girdy),          // --->/\
   // Setup
   .imiso      (imiso_gw),
   .imosi      (jmosi),
   .intpt_fini (intpt_gw_fini),
   .intpt_timo (intpt_gw_timo),
   .flag       (flag_out),
   .myID       (myID),
   // Global
   .reset      (tile_reset),
   .clock      (tile_clock)
   );

// Bus Collation ----------------------------------------------------------------------------------------------------------------

assign   imiso       =  imiso_lc[`IMISO_WIDTH-1:0]                            // local
                     |  imiso_sh[`IMISO_WIDTH-1:0]                            // sha256
                     |  imiso_xs[`IMISO_WIDTH-1:0]                            // salsa
                     |  imiso_ar[`IMISO_WIDTH-1:0]                            // A2Sreset state reads
                     |  imiso_gw[`IMISO_WIDTH-1:0];                           // gateway

assign   brd0[31:0]  =  bmiso_cm[31:00]                                       // code memory
                     |  bmiso_dm[31:00]                                       // data memory
                     |  bmiso_wm[63:32]                                       // work memory
                     |  imiso   [31:00],                                      // -- above --

         brd1[31:0]  =  bmiso_wm[31:00];                                      // upper data from work memory

// Give and Take Signalling -----------------------------------------------------------------------------------------------------

assign   BMk         =  bmiso_cm[32]   |  bmiso_dm[32] | bmiso_wm[64],        // Memory   Acknowledges
         IOk         =          sel_IO &  imiso[32],                          // Register Acknowledges back to A2S
         JOk         =  ROk  | ~sel_IO &  imiso[32],                          // Register Acknowledges back to XMOSI/XMISO
         WMoff       =  SalsaOn,    // |  MPPon,                              // WM off-limits to outside!
         ERk         =  WMoff &  eWM   |  EBk;                                // Error Acknowledge

assign   bordy       =  BMk   |  JOk   |  ERk  | enA2SRST;                    // Access result ready to output

// Collate bmiso_rsp
assign   amiso_rs    =  ERk ? {2'b01,acmd[5:0], atag, asrc, myID, awrd, aadr }   // Error as response force src as self
                            : {1'b00,acmd[5:0], atag, asrc, myID, brd0, brd1 };  // Set as response & Swap src and dst
//  Little 2-way arbiter
assign   sel_gw      =  GW_LAST  ?  ~bordy & gordy : gordy;
always @(posedge tile_clock) if (tile_reset || airdy && (bordy || gordy)) GW_LAST <= `tCQ ~tile_reset & sel_gw;

assign   amiso       =  sel_gw   ? amiso_rq[`XMISO_WIDTH-3:0]
                                 : amiso_rs[`XMISO_WIDTH-3:0],
         apush       =  sel_gw   ? gordy : bordy;                             // push request or response to output

// Clock Domain Crossing --------------------------------------------------------------------------------------------------------

A2_CDC i_CDCo  (                 //                                                              :
   // Input Side -- from Tile    //                                                 TILE    +----:----+
   .CDCir   (airdy),             // OUTPUT - Input ready    (FIFO not full)         <-------|    :    |
   .wCDC    (apush),             // INPUT  - Write          (FIFO push)             ------->|    :    |
   .CDCld   (ld_xmiso),          // OUTPUT - Load  input    (Load XMISO register)           |    :    |------> load input
   .ireset  (tile_reset),        // INPUT    reset synchronized to iclock           ------->|    :    |
   .iclock  (tile_clock),        // INPUT    clock                                  --------|>   :    |
   // Output Side -- to Raceway  //                                                         |   CDC   |      RACEWAY
   .rCDC    (xmiso_fb),          // INPUT    Read           (FIFO pop)                      |    :    |<------
   .CDCor   (xordy),             // OUTPUT   Output Ready   (FIFO not empty)                |    :    |------>
   .oreset  (xinit),             // INPUT    Reset synchronized to Tile clock               |    :    |<------
   .oclock  (xclock)             // INPUT    Tile clock                                     |    :   <|-------
   );                            //                                                         +----:----+
                                 //                                                              :

assign   gpop     =  sel_gw   &  airdy,                                       // pop output request from gateway
         bpop     = ~sel_gw   &  airdy;                                       // pop access request from input

always @(posedge tile_clock) if (ld_xmiso) XMISO[`XMISO_WIDTH-3:0] <= `tCQ amiso[`XMISO_WIDTH-3:0];
                                           //
// Collate xmiso                          //_clock domain crossing: XMISO loaded before the CDC circuit above can send 'xordy'!
assign   xmiso_clk   =  xclock;          //
assign   xmiso       =  { 1'b0, xordy,  XMISO };      // RTT:  We need to add the timeout to bit 97

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
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
`ifdef   verify_tile_one

//`define  CHECK_CODE_MEMORY
//`define  CHECK_DATA_MEMORY
//`define  CHECK_WORK_MEMORY
//`define  SHOW_FIRST_64
//`define  CHECK_SHA256_ENGINE
//`define  CHECK_XSALSA_ENGINE
`define  CHECK_GATEWAY
`define  CHECK_A2S_SOFTWARE
`define  HIGH_SPEED_TEST

`include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/COGNITUM_IO_addresses.tbh"

module   t;

localparam  XCK_PERIOD     =  0.800;   // 1.25GHx = picoseconds

localparam  [7:0] myID     =  8'd42;   // 0x2a

localparam  CODESIZE       = `TILEONE_CODESIZE; //Bytes
localparam  DATASIZE       = `TILEONE_DATASIZE; // Bytes
localparam  WORKSIZE       = `TILEONE_WORKSIZE; // Bytes  (512 x 1024-bits)

localparam  WIDTH_RACEWAY  =  99;               // Width of Raceway (bits)

//localparam  initialcode    =  "..\\..\\asm\\TileOneBase_code.rmh";
//localparam  initialdata    =  "..\\..\\asm\\TileOneBase_data.rmh";
localparam  initialcode    =  "..\\..\\obj\\TileOneBase_code.rmh";
localparam  initialdata    =  "..\\..\\obj\\TileOneBase_data.rmh";

localparam  DCMW           =  CODESIZE / 4;
localparam  DDMW           =  DATASIZE / 4;
localparam  USEDSIZE       =  32'h0650;

// Tile Data Memory addresses
localparam  [31:0]   HEADERbase  =  32'h40002200,
                     NONCE       =  HEADERbase   +   76,    // Nonce is first word of header (big endian)
                     lock        =  NONCE        +    4,    // lock word for secure communication with TileZero
                     increment   =  lock         +    4,    // Nonce Increment value
                     ScryptTarget=  increment    +    4,    // target vale to compare to Scrypt result
                     nonce_end   =  ScryptTarget +    4,    // Pointer to Nonce_End vector (1 per Tile)
                     golden      =  nonce_end    + 1024;    // Pointer to Golden Nonce vector (1 per Tile)

`include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2Sv2r3_ISA_params.vh"
`include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2Sv2r3_Registers.vh"
`include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2Sv2r3_ISAstring.vh"

localparam  IDLE = 0, SEND = 1, WAIT = 2, DONE = 3, TIME = 4;
reg   [95:0]   stimulus, response;
reg   [31:0]   XFSMstring;
reg   [ 2:0]   XFSM;
reg            start, finish;
integer        wait4irdy, wait4ordy;
always @* casez (XFSM)
   0:       XFSMstring  =  "IDLE";
   1:       XFSMstring  =  "SEND";
   2:       XFSMstring  =  "WAIT";
   3:       XFSMstring  =  "DONE";
   default  XFSMstring  =  "TIME";
   endcase

localparam [4:0] START = 1, VECTOR= 2, FETCH= 4, NEXT = 8;  // FSM States
reg [39:0]  FSMstring;
always @* casez (mut.i_A2S.FSM)
   START :  FSMstring  =  "START";
   VECTOR:  FSMstring  =  "VECTR";
   FETCH :  FSMstring  =  "FETCH";
   default  FSMstring  =  "NEXT ";
   endcase

localparam [2:0]  IDLe = 0, PREP = 1, LOW = 2, HIGH = 3, TAIL = 4,  REST = 5, DONe = 6;
reg [31:0]  SSMstring;
always @* casez (mut.i_Salsa.FSM)
   IDLe  :  SSMstring  =  "IDLE";
   PREP  :  SSMstring  =  "PREP";
   LOW   :  SSMstring  =  "LOW ";
   HIGH  :  SSMstring  =  "HIGH";
   TAIL  :  SSMstring  =  "TAIL";
   REST  :  SSMstring  =  "REST";
   DONe  :  SSMstring  =  "DONE";
   default  SSMstring  =  "----";
   endcase


wire  [31:0]   orderS    = ISAstring(mut.i_A2S.FSM[3:0], mut.i_A2S.order[5:0], mut.i_A2S.fn[15:0], mut.i_A2S.YQmt, 1'b0 );
wire  [31:0]   slot0     = ISAstring(mut.i_A2S.FSM[3:0], mut.i_A2S.I[33:28],   mut.i_A2S.fn[15:0], mut.i_A2S.YQmt, 1'b1 );
wire  [31:0]   slot1     = ISAstring(mut.i_A2S.FSM[3:0], mut.i_A2S.I[27:22],   mut.i_A2S.fn[15:0], mut.i_A2S.YQmt, 1'b1 );
wire  [31:0]   slot2     = ISAstring(mut.i_A2S.FSM[3:0], mut.i_A2S.I[21:16],   mut.i_A2S.fn[15:0], mut.i_A2S.YQmt, 1'b1 );
wire  [31:0]   slot3     = ISAstring(mut.i_A2S.FSM[3:0], mut.i_A2S.I[15:10],   mut.i_A2S.fn[15:0], mut.i_A2S.YQmt, 1'b1 );
wire  [31:0]   slot4     = ISAstring(mut.i_A2S.FSM[3:0], mut.i_A2S.I[ 9:4 ],   mut.i_A2S.fn[15:0], mut.i_A2S.YQmt, 1'b1 );
wire  [ 5:0]   slot4h    =                               mut.i_A2S.I[ 9:4 ];
wire  [ 3:0]   slot5     =                               mut.i_A2S.I[ 3:0 ];

integer        i,j,  testnum, salsacount, error,count;
reg            TIMEOUT;

reg  [          1023:0] stim, resp;
reg  [           511:0] mesg;
reg  [           255:0] hash;
reg  [           255:0] xpct;
reg  [            31:0] expect;
reg  [           639:0] header, header2;
reg  [            31:0] address, data;

reg  [`XMOSI_WIDTH-1:0] xmosi;       // Raceway to Tile Slave Input
wire                    xmosi_fb;    // feedback xirdy
wire                    xmosi_clk;
wire [`XMISO_WIDTH-1:0] xmiso;       // Tile Slave Output to Raceway
reg                     xmiso_fb;   //  feedback xpop

reg                     xclock;
wire                    xclockr;
reg                     xpush;
reg                     xresetn;
reg                     flag_in;
wire                    flag_out;
reg                     debug;

initial begin
   xclock   =  0;
   forever
      xclock      = #(XCK_PERIOD/2) ~xclock;
   end

assign   xmosi_clk = xclock;

TileOne #(CODESIZE,DATASIZE,WORKSIZE) mut (
   // System Interconnect
   // Output from Tile              97       96     95:00
   .xmiso    (xmiso[97:0]),      // xresetr, xordy, xmiso
   .xmiso_clk(xclockr),          // xclockr
   .xmiso_fb (xmiso_fb),         // xpop
   // Input into Tile               97       96     95:00
   .xmosi    (xmosi[97:0]),      // xresetn, xpush, xmosi
   .xmosi_clk(xmosi_clk),        // xclock
   .xmosi_fb (xmosi_fb),         // xirdy
   // Global
   .flag_out (flag_out),
   .flag_in  (flag_in ),
   .myID     (myID)              // Tile ID for use with 'src' and 'dst' fields
   );

// Tile Access State Machine -- Mimicing the Raceway

always @* xmosi[97] = xresetn;

reg   active;
always @(posedge xclock)
   if (!xresetn) begin
      XFSM                 <= IDLE;
      wait4irdy            <= 0;
      wait4ordy            <= 0;
      TIMEOUT              <= 0;
      active               <= 1'b0;
      end
   else case (XFSM)
      IDLE: if (start) begin // xpush
               xmosi[96:0] <= { 1'b1, stimulus};
               active      <= 1'b1;
               wait4irdy   <= 0;
               wait4ordy   <= 0;
               if (debug) $display("           Start Access ------------------------------");
               XFSM        <= SEND;
               end
      SEND: if (wait4irdy >= 100) begin
               $display("           Timed-out -- Requested Device Not Ready! or Doesn't Exist");
               XFSM        <= TIME;
               end
            else if ((stimulus[95:88] == `BCTWR)
               ||    (stimulus[95:88] == `BCTRL)
               ||    (stimulus[95:88] == `BCTRS)
               ||    (stimulus[95:88] == `UNIWK)) begin
               xmosi[96:0] <= { 1'b0, stimulus};
               XFSM        <= DONE;
               end
            else if (xmosi_fb) begin
               xmosi[96:0] <= { 1'b0, stimulus};
               wait4irdy   <= 0;
               XFSM        <= WAIT; // !xmosi[95] || xmosi[93] ? DONE : WAIT;   // If response or broadcast end the access, else wait
               end
            else
               wait4irdy   <= wait4irdy + 1;

      WAIT: if (wait4ordy > 500) begin
               $display("           Timed-out -- No Response");
               XFSM        <= TIME;
               end
            else if (xmiso[96]) begin  // xordy
               xmiso_fb    <= 1'b1;
               if (debug)     $display("           Response found:  %h",xmiso[95:0]);
               if (xmiso[94]) $display("           Error1:  Found an error response");
               response    <= xmiso[95:0];
               finish      <= 1'b1;
               XFSM        <= DONE;
               end
            else
               wait4ordy   <= wait4ordy + 1;

      DONE: begin
            finish         <= 1'b0;
            xmiso_fb       <= 1'b0;
            wait4irdy      <= 0;
            wait4ordy      <= 0;
            XFSM           <= IDLE;
            active         <= 1'b0;
            end

      default begin
            TIMEOUT        <= 1;
            XFSM           <= IDLE;
            active         <= 1'b0;
            end
      endcase


reg   [95:0]   XMOSI;
reg            XPUSH;
wire  [31:0]   result         = xmiso[63:32];
wire  [63:0]   double_result  = xmiso[63:00];

reg   [31:0]   shadow_code [0:DCMW-1];
reg   [31:0]   shadow_data [0:DCMW-1];

reg   SCR_change;
always @(posedge mut.tile_clock) begin
   SCR_change <= (mut.rc_SCR || mut.ld_SCR || mut.sw_SCR);
   if (SCR_change) begin
//    $display($time,": Scratch State = %h \"%s\" SHA256 Message = %h ",
//       mut.SCRATCH,mut.SCRATCH, mut.i_shaCoP.M);
      $display("           Scratch State = %h \"%s\" ",
         mut.SCRATCH,mut.SCRATCH,);
      end
   end

always @(posedge mut.i_shaCoP.clock) begin
   `tCQ
   if ((mut.i_shaCoP.FSM==0) && mut.i_shaCoP.go) begin
      $display("           iHASH = %h",mut.i_shaCoP.S);
      $display("           iMESG = %h",mut.i_shaCoP.M);
      end
   if ((mut.i_shaCoP.FSM==5) && mut.i_shaCoP.fini) begin
      $display("           HASHo = %h",mut.i_shaCoP.S);
      end
   end

always @(posedge mut.i_Salsa.fast_clock)     //Steps.
   if (!xresetn)  count  <= 0;               //Steps.
   else           count  <= count +1;        //Steps.

always @(posedge xclock)
   if (mut.i_Salsa.idle)   salsacount  <= 0;
   else                    salsacount  <= salsacount +1;

always @(posedge mut.i_Salsa.fast_clock) begin
   `tFCQ
   if ((mut.i_Salsa.FSM==0) && mut.i_Salsa.fast_start) begin
      for (i=0; i<4; i=i+1) begin
         if (i==0) $write("           Xbuf = ");
         else      $write("                  ");
         for (j=0; j<8; j=j+1)
            $write("r%2d=%h ",(8*i+j),mut.i_Salsa.r0_X[8*i+j]);
         $display(" ");
         end
      $display("\n");
      end
   if ((mut.i_Salsa.FSM==6) && mut.i_Salsa.fast_fini) begin
      for (i=0; i<4; i=i+1) begin
         if (i==0) $write("           Xres = ");
         else      $write("                  ");
         for (j=0; j<8; j=j+1)
            $write("r%2d=%h ",(8*i+j),mut.i_Salsa.r0_X[8*i+j]);
         $display(" ");
         end
      $display("\n");
      end
   end

wire  [31:0]   instr = mut.i_A2S.I[33:2];

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// THE MAIN TESTS                                                                                                              //
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

initial begin
   $display(" Tile One Verification --------------------------------------------------------------------------------------\n\n");
   $readmemh(initialcode,shadow_code,0,DCMW-1);
   $display(" Shadow Code File loaded");
   for (i=0; i<256; i=i+4) $display(" %h %h %h %h",shadow_code[i],shadow_code[i+1],shadow_code[i+2],shadow_code[i+3]);
   $readmemh(initialdata,shadow_data,0,DDMW-1);
   $display(" Shadow Data File loaded");
   for (i=0; i<64; i=i+4) $display(" %h %h %h %h",shadow_data[i],shadow_data[i+1],shadow_data[i+2],shadow_data[i+3]);
   xmiso_fb =  1'b0;
   xresetn  =  1'b1;
   flag_in  =  1'b0;
   debug    =  1'b1;
   repeat (50) @(posedge xclock) `tCQ;
   xresetn  =  1'b0;
   repeat (50) @(posedge xclock) `tCQ;
   xresetn  =  1'b1;
   repeat (50) @(posedge xclock) `tCQ;

   // Try writing to the Ring Oscillator ---------------------------------------------------------------------------------------
   testnum  =  1;
   $display(" Test %2d:  Set Ring Oscillator to lowest frequency & divider to 3",testnum);
   //          CMD     TAG    DST   SRC    DATA          ADDR
   TileAccess({`SYSWR, 8'h00, myID, 8'h00, 32'h00000000, `aTILE_SET_CLOCKS});    // Ensure RO is not 'x's
   repeat (10) @(posedge xclock) `tCQ;
   //          CMD     TAG    DST   SRC    DATA          ADDR
   TileAccess({`SYSWR, 8'h00, myID, 8'h00, 32'h0000003f, `aTILE_SET_CLOCKS});
   repeat (10) @(posedge xclock) `tCQ;

   testnum  =  2;
   $display(" Test %2d:  Set clocks to default",testnum);
   repeat (20) @(posedge xclock) `tCQ;
   $display("           Set all clocks to xclock");
   TileAccess({`SYSWR, 8'h00, myID, 8'h00, 32'h00000000, `aTILE_SET_CLOCKS});
   repeat (10) @(posedge xclock) `tCQ;

   // Try an access to 'nothing' -----------------------------------------------------------------------------------------------
   testnum  =  3;
   $display(" Test %2da: Try access to unused location",testnum);
   //          CMD,   TAG   DST    SRC            DATA          ADDR
   TileAccess({8'h80, 8'h00, myID, 8'h00, 32'h00000000, 32'h00000000});

   repeat (10) @(posedge xclock) `tCQ;

   $display(" Test %2db: Try a spurious response",testnum);
   //          CMD,   TAG    DST    SRC            DATA          ADDR
   TileAccess({8'h0c, 8'h0b, 8'h0a, 8'h09, 32'h08070605, 32'h04030201});
   repeat (10) @(posedge xclock) `tCQ;

   $display(" Test %2dc: Try access to unused location",testnum);
   //          CMD,   TAG    DST    SRC            DATA          ADDR
   TileAccess({8'h91, 8'h00, myID, 8'h00, 32'hffffffff, 32'hc0005fc0});
   repeat (10) @(posedge xclock) `tCQ;

   $display(" Test %2dd: Try access to unused location",testnum);
   //          CMD,   TAG    DST    SRC            DATA          ADDR
   TileAccess({8'h91, 8'h00, myID, 8'h00, 32'h00000000, 32'hc0005fc0});
   // Allow time for timeout
   repeat (100) @(posedge xclock) `tCQ;

   debug    =  1'b0;

   // Try writing to the Code Memory -------------------------------------------------------------------------------------------

`ifdef   CHECK_CODE_MEMORY
   testnum  =  4;
   $display(" Test %2d:  Test Code Memory",testnum);
   $write  ("           ");
   for (i=0; i<CODESIZE; i=i+4) begin
      address = i;
      TileAccess({`UNIWR, 8'h00, myID, 8'h00, address, address});
      if (i%512==0) $write(".");
      end
   $display("\n Test %2d:  Code Memory loaded",testnum);

   testnum  =  5;
   $display("\n Test %2d:  Verify Code Memory",testnum);
   $write  ("           ");
   for (i=0; i<CODESIZE; i=i+4) begin
      address = i;
      TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h00000000, address});
      if (result !== address) $display(" Error: Wrong response -- Expected %h  Found %h\n            ",address,result);
      if (i%512==0) $write(".");
      end

   if (result !== (CODESIZE-4)) $display(" Error: Code Memory Depth Error -- Expected %h  Found %h\n            ",CODESIZE-4,result);
   $display("\n           Code Memory Test Complete");

`endif   //CHECK_CODE_MEMORY

   // Try writing to the Data Memory -------------------------------------------------------------------------------------------

`ifdef   CHECK_DATA_MEMORY
   testnum  =  6;
   $display(" Test %2d:  Test Data Memory",testnum);
   $write  ("           ");
   for (i=0;i<DATASIZE; i=i+4) begin
      address = 32'h40002000 + i;
      TileAccess({`UNIWR, 8'h00, myID, 8'h00, ~address, address});
      if (i%512==0) $write(".");
      end
   testnum  =  7;
   $display("\n Test %2d:  Verify Data Memory",testnum);
   $write  ("           ");
   for (i=0;i<DATASIZE; i=i+4) begin
      address = 32'h40002000 + i;
      TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h00000000, address});
      if (result !== ~address) $display(" Error: Wrong response -- Expected %h  Found %h\n            ",~address,result);
      if (i%512==0) $write(".");
      end

   if (result !== ~(32'h40002000 + DATASIZE-4))
      $display(" Error: Data Memory Depth Error -- Expected %h  Found %h\n            ",~(32'h40000000+DATASIZE-4),result);

   $display("\n           Data Memory Test Complete");
`endif   //CHECK_DATA_MEMORY

   // Try writing to the Work Memory -------------------------------------------------------------------------------------------

`ifdef   CHECK_WORK_MEMORY
   testnum  =  8;
   $display(" Test %2d:  Test Work Memory",testnum);
   $display("           Loading Work Memory");
   $write  ("           ");
   for (i=0;i<WORKSIZE; i=i+4) begin
      address = 32'h80010000 + i;
      TileAccess({`UNIWR, 8'h00, myID, 8'h00, address, address});
      if (i%1024==0) $write(".");
      end
   testnum  =  9;
   $display("\n Test %2d:  Verifying Work Memory",testnum);
   $write  ("           ");
   for (i=0;i<WORKSIZE; i=i+8) begin
      address = 32'h80010000 + i;
      TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h00000000, address});
      error   = 0;
      if (double_result !== {address[31:3],3'd0,address[31:3],3'd4}) begin
         error = 1;
         $display(" Error: Wrong response -- Expected %h  Found %h @ address %h",
            {address[31:3],3'd0,address[31:3],3'd4},double_result,address);
         end
      if (i%1024==0) $write(".");
      end

   if (result !== (32'h80010000 + WORKSIZE-8))
                        $display(" Error: Work Memory Depth Error -- Expected %h  Found %h",(32'h80000000+WORKSIZE-8),result);

   $display("\n           Test Work Memory Complete");
`endif   //CHECK_WORK_MEMORY

   // Try writing to the SHA256 Coprocessor Registers----------------------------------------------------------------------------

`ifdef   CHECK_SHA256_ENGINE
   testnum  =  10;
   $display(" Test %2d:  Test SHA256 Engine",testnum);
   xpct  =   256'h0b28938404c4bd4ac42dd095b0921451593a803eaf6ddfcff9e3fe4dfaae9656;
   hash  =   256'h5be0cd191f83d9ab9b05688c510e527fa54ff53a3c6ef372bb67ae856a09e667;
   mesg  =  {256'hc791d4646240fc2a2d1b80900020a24dc501ef1599fc48ed6cbac920af755756,
             256'h18e7b1e8eaf0b62a90d1942ea64d250357e9a09c063a47827c57b44e01000000};

   TileAccess({`UNIWR, 8'h00, myID, 8'h00, 32'h0, `aTILE_SHA256_WAKE});

   for (i=0; i<16; i=i+1)
      TileAccess({`UNIWR, 8'h00, myID, 8'h00, mesg[32*i +: 32],`aTILE_SHA256_MESG});

   for (i=0; i<16; i=i+1)  begin
      expect = mesg[32*i+:32];
      TileAccess({`UNIRD,8'h00,myID,8'h00,32'h0,`aTILE_SHA256_MESG}); if (result!==expect)
         $display(" Error : Message Read back -- Expected %h  Found %h", expect,result);
      end

   testnum  =  11;
   $display(" Test %2d:  Verify SHA256 Setup",testnum);
   for (i=0; i<8; i=i+1)
      TileAccess({`UNIWR, 8'h00, myID, 8'h00, hash[32*(7-i)  +: 32], `aTILE_SHA256_HASH});

   for (i=0; i<8; i=i+1)   begin
      expect = hash[32*(7-i)+:32];
      TileAccess({`UNIRD,8'h00,myID,8'h00,32'h0,`aTILE_SHA256_HASH}); if (result!==expect)
         $display(" Error : Hash  read back   -- Expected %h  Found %h", expect,result);
      end

   testnum  =  12;
   $display(" Test %2d:  Verify SHA256 Results",testnum);
   // Initiate SHA256 algorithm
   TileAccess({`UNIWR, 8'h00, myID, 8'h00, 32'h00000000, `aTILE_SHA256_STRT_RDY});
   // Scan for finish
   while (result == 0)     TileAccess({`UNIRD,8'h00,myID,8'h00,32'h0,`aTILE_SHA256_STRT_RDY});

   for (i=0; i<8; i=i+1)   begin
      expect = xpct[32*(7-i)+:32];
      TileAccess({`UNIRD,8'h00,myID,8'h00,32'h0,`aTILE_SHA256_HASH}); if  (result !== expect)
         $display(" Error : Digest read       -- Expected %h  Found %h",expect,result);
      end

   $display("           Test SHA256 Complete");

   xpct  =  256'hf20015adb410ff6196177a9cb00361a35dae2223414140de8f01cfeaba7816bf;
   hash  =  256'h5be0cd191f83d9ab9b05688c510e527fa54ff53a3c6ef372bb67ae856a09e667;
   mesg  = {256'h6162638000000000000000000000000000000000000000000000000000000000,
            256'h0000000000000000000000000000000000000000000000000000000000000018};

   testnum  =  13;
   $display(" Test %2d:  Try a different SHA256 Test",testnum);

   runSHA256;

`endif   //CHECK_SHA256_ENGINE

   // Try writing to Xsalsa Registers -------------------------------------------------------------------------------------------

`ifdef   CHECK_XSALSA_ENGINE
   testnum  =  14;
   $display(" Test %2d:  Test Xsalsa20/8", testnum);
   stim  = {256'h0c809ebc35e7e5b9308736de918332c80c2eb1fcc348c7c76b5daa0ee44de053,
            256'h6d04b6621cb651959328032861663eabbf005a0ca327784bb813398e7dd42df6,
            256'h917498556eb884b7899f30bb956a5647fcb5076e2fc4f9e579f1e7fcda315878,
            256'h6155e74c3c2089d26ff087e9e883b1e55174c638fcff36fb0793d68d_5d9ec5df};

   resp  = {256'hb8dc6d17a70820da7795cdcfc42520c502c1f36a95dfe383b706642ab1c70f4d,
            256'hd3a36645b0bf693b869274126ab91d19f4a6bcfc588a674ad56c223817edfc52,
            256'hd60adf34d792e0662797a268c1eebae593a97f4b19c2fa4fea973768f4e6db64,
            256'h1a693b27379e14f043d980c101409b1bdd9e7aab2550adcdd87b8a02861550dc};

   // Enable WM access
   TileAccess({`UNIWR, 8'h00, myID, 8'h00, 32'h00000011,`aTILE_XSALSA_CTRL});

   for (i=0; i<32; i=i+1)
      TileAccess({`UNIWR, 8'h00, myID, 8'h00, stim[32*i +: 32],`aTILE_XSALSA_XBUF});

   TileAccess({`UNIWR, 8'h00, myID, 8'h00, 32'hffffffff,`aTILE_XSALSA_STRT_RDY});

   while (result == 0)
      TileAccess({`UNIRD,8'h00,myID,8'h00,32'h0,`aTILE_XSALSA_STRT_RDY});

   testnum  =  15;
   $display(" Test %2d:  Verify Xsalsa20/8 Results", testnum);
   for (i=0; i<32; i=i+1)   begin
      expect = resp[32*i+:32];
      TileAccess({`UNIRD,8'h00,myID,8'h00,32'h0,`aTILE_XSALSA_XBUF});
      if (result !== expect)
         $display("           Error : Digest read -- Expected %h  Found %h",expect,result);
      end

   // Disable WM access
   TileAccess({`UNIWR, 8'h00, myID, 8'h00, 32'h00000000,`aTILE_XSALSA_CTRL});

//   for (i=0;i<WORKSIZE; i=i+8) begin
//      address = 32'h80010000 + i;
//      TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h00000000, address});
//      $display("WM %h %h ", address[31:0],double_result);
//      end

   $display("           Test Xsalsa20/8 Complete");

`endif   //CHECK_XSALSA_ENGINE

// Check the Gateway ------------------------------------------------------------------------------------------------------------
`ifdef   CHECK_GATEWAY

   debug = 1'b1;
   testnum  =  16;
   $display(" Test %2d:  Test Gateway",testnum);

   // Mimic CPU generating a request
   TileAccess({`UNIWR, 8'h01, myID, 8'h00, 32'hdeadbeef,`aTILE_GATEWAY_ADDR});
   TileAccess({`UNIWR, 8'h02, myID, 8'h00, 32'habadf1d0,`aTILE_GATEWAY_DATA});
   TileAccess({`UNIWR, 8'h03, myID, 8'h00, 32'h90000000,`aTILE_GATEWAY_CMND});   // load Command kicks off an outgoing request!
   $display("           Regs loaded");

   while (!(xmiso[96] && xmiso[95])) @(posedge xclock) `tCQ;                     // Spin while not ordy or not a request
   $display("           Wait for Incoming request");
   repeat ( 1) @(posedge xclock) `tCQ;
   if (xmiso[94]) $display(  "           Error2:  Found an error response");
   else           $display("\n           INCOMING REQUEST RECEIVED \n");
   $display("              COMMAND = %h",  xmiso[95:64]);
   $display("              RD_DATA = %h",  xmiso[63:32]);
   $display("              ADDRESS = %h\n",xmiso[31:0 ]);
   xmiso_fb =  1'b1;
   @(posedge xclock) `tCQ;
   xmiso_fb =  1'b0;

   $display("           Send a Response");
   // Send Tile an acknowledge of the incoming request
   TileAccess({`UNIWK, 8'h00, myID, 8'h00, 32'h600dbeef, 32'h0 });         // This goes to Gateway to close the access
   repeat (5) @(posedge xclock);
   // No response back expected

   $display("           Mimic a CPU Response Read");
   // Mimic CPU reading response
   TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h600df1d0,`aTILE_GATEWAY_RESP});
   repeat (5) @(posedge xclock);
   TileAccess({`UNIWR, 8'h01, myID, 8'h00, 32'h00000000,`aTILE_GATEWAY_ADDR});
   TileAccess({`UNIWR, 8'h02, myID, 8'h00, 32'h00000000,`aTILE_GATEWAY_DATA});

   $display("           Gateway test complete");
   debug = 1'b0;

`endif //CHECK_GATEWAY
// Load Code and Data Memory ----------------------------------------------------------------------------------------------------
`ifdef   CHECK_A2S_SOFTWARE

   testnum  =  17;
   $display(" Test %2d:  Load Code Memory",testnum);
   for (i=0; i<USEDSIZE/4; i=i+1) begin
      address = 32'h00000000 + 4*i;
      TileAccess({`UNIWR, 8'h00, myID, 8'h00, shadow_code[i], address});
      end

   testnum  =  18;
   $display(" Test %2d:  Verify Code",testnum);
   for (i=0; i<USEDSIZE/4; i=i+1) begin
      address = 32'h00000000 + 4*i;
      TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h00000000, address});
      if (result !== shadow_code[i]) $display(" Error: Wrong response -- Expected %h  Found %h",shadow_code[i],result);
      end

   testnum  =  19;
   $display(" Test %2d:  Load Data Memory",testnum);
   for (i=0; i<USEDSIZE/4; i=i+1) begin
      address = 32'h40002000 + 4*i;
      TileAccess({`UNIWR, 8'h00, myID, 8'h00, shadow_data[i], address});
      end

   testnum  =  20;
   $display(" Test %2d:  Verify Data",testnum);
   for (i=0; i<USEDSIZE/4; i=i+1) begin
      address = 32'h40002000 + 4*i;
      TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h00000000, address});
      if (result !== shadow_data[i]) $display(" Error: Wrong response -- Expected %h  Found %h",shadow_data[i],result);
      end

`ifdef   SHOW_FIRST_64
   $display("           First 64 of Code");
   for (i=0; i<16; i=i+1) begin
      address = 16*i;
      TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h00000000, address});
      $write  ("           %h", result);
      TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h00000000, address+4});
      $write  (" %h", result);
      TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h00000000, address+8});
      $write  (" %h", result);
      TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h00000000, address+12});
      $display(" %h", result);
      end
   $display("           First 64 of Data Memories");
   for (i=0; i<16; i=i+1) begin
      address = 32'h40002000 + 16*i;
      TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h00000000, address});
      $write  ("           %h", result);
      TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h00000000, address+4});
      $write  (" %h", result);
      TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h00000000, address+8});
      $write  (" %h", result);
      TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h00000000, address+12});
      $display(" %h", result);
      end
`endif   //SHOW_FIRST_64
   $display("           Code and Data Memories Loaded ");

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// The BIG TEST                                                                                                               //
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

   // Nota Bene the Nonce V is one lower than we need. Should be 0000318f but the algorithm starts with a nonce increment!!
   header = { 320'h0000318d7e71441b141fe951b2b0c7dfc791d4646240fc2a2d1b80900020a24dc501ef1599fc48ed,
              320'h6cbac920af75575618e7b1e8eaf0b62a90d1942ea64d250357e9a09c063a47827c57b44e01000000};

   testnum  =  21;
   $display(" Test %2d:  Load a Scrypt Test",testnum);
   for (i=0; i<20; i=i+1) begin
      address = HEADERbase + i*4;
      TileAccess({`UNIWR, 8'h00, myID, 8'h00, header[32*i +:32], address});
`define  SHOW_HEADER
`ifdef   SHOW_HEADER
      $display("           Writing header %h to address %h", header[32*i +:32], address );
`endif   //SHOW_HEADER
      end

   TileAccess({`UNIWR, 8'h00, myID, 8'h00, 32'h00000000, lock        });
   TileAccess({`UNIWR, 8'h00, myID, 8'h00, 32'h00000001, increment   });
   TileAccess({`UNIWR, 8'h00, myID, 8'h00, 32'h000003ff, ScryptTarget});
   TileAccess({`UNIWR, 8'h00, myID, 8'h00, 32'h00010000, nonce_end   });
   TileAccess({`UNIWR, 8'h00, myID, 8'h00, 32'h00010100, golden      });

   testnum  =  22;
   $display(" Test %2d:  Let 'er rip!! ",testnum);
   $display("            Release A2S ");

   // Clear A2S reset to allow processor to run:  No acknowledge
   TileAccess({`BCTRL, 8'h00, 8'h00, 8'h00, 32'h00000000, `aTILE_A2S_RESET});

   testnum  =  23;
   $display(" Test %2d:  Verify Scrypt Completion",testnum);

   // At Scrypt completion and a Golden Nonce found wait for an incoming Gateway request
   while (!xmiso[96] || !xmiso[95]) @(posedge xclock) `tCQ;
   repeat ( 1) @(posedge xclock) `tCQ;
   xmiso_fb =  1'b1;

   if (xmiso[94]) $display("           Error3:  Found an error response");
   else $display("\n Test %2d:   INCOMING REQUEST RECEIVED \n",testnum);

   while (xmiso[96]) @(posedge xclock) `tCQ;          // Spin while not ordy or not a request
   @(posedge xclock) `tCQ;
   xmiso_fb =  1'b0;
   $display("              COMMAND = %h",  xmiso[95:64]);
   $display("              RD_DATA = %h",  xmiso[63:32]);
   $display("              ADDRESS = %h\n",xmiso[31:0 ]);

   if (xmiso[63:32] == 32'h0000318f) $display("              RD_DATA %h is a Golden Nonce\n\n",xmiso[63:32]);

   repeat (10) @(posedge xclock);

   // Send Tile an acknowledge of the incoming request
   $display("           Send an Acknowledge");
   TileAccess({`UNIWK, 8'h00, myID, 8'h00, 32'h600dbeef, 32'h0 });         // This goes to Gateway to close the access
   repeat (10) @(posedge xclock) `tCQ;

////// Moved to real code!!!!
// // Mimic CPU reading response
// $display("           Mimic a CPU Response Read");
// TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h600df1d0,`aTILE_GATEWAY_RESP});
// if (xmiso[63:32] != 32'h600dbeef) $display("              Response not taken, found %h\n\n",xmiso[63:32]);
//
// repeat (5) @(posedge xclock);
// $display("           Clear out the Gateway");
// TileAccess({`UNIWR, 8'h01, myID, 8'h00, 32'h00000000,`aTILE_GATEWAY_ADDR});
// TileAccess({`UNIWR, 8'h02, myID, 8'h00, 32'h00000000,`aTILE_GATEWAY_DATA});

//   repeat (5000) @(posedge xclock);
//
//   // Reset the A2S with a Broadcast reset -- so no acknowledge
//   TileAccess({`BCTRS, 8'h00, 8'h00, 8'h00, 32'hffffffff, `aTILE_A2S_RESET});

   repeat (10000) @(posedge xclock) `tCQ;

`endif   //CHECK_A2S_SOFTWARE

// Clock Speed ------------------------------------------------------------------------------------------------------------------

`ifdef   HIGH_SPEED_TEST

   testnum  =  24;
   debug    =  1'b1;
   $display(" Test %2d:  First shut down the Tile ",testnum);
   TileAccess({`BCTRS, 8'h00, 8'h00, 8'h00, 32'h00000001, `aTILE_A2S_RESET});
   TileAccess({`UNIWR, 8'h00,  myID, 8'h00, 32'h00000001, `aTILE_XSALSA_STOP});
   TileAccess({`UNIWR, 8'h00,  myID, 8'h00, 32'h00000001, `aTILE_SHA256_STOP});

   testnum  =  25;
   $display(" Test %2d:  Try full speed ",testnum);
   $display("           Set Ring Oscillator to highest frequency");
   TileAccess({`SYSWR, 8'h00, myID, 8'h00, 32'h00000048, `aTILE_SET_CLOCKS});

   repeat (100) @(posedge xclock);

   $display(" Test %2d:  Set Salsa On - No wait state",testnum);
   TileAccess({`UNIWR,        8'h00, myID, 8'h00, 32'h00000001, `aTILE_XSALSA_CTRL});

   // Nota Bene the Nonce V is one lower than we need.
   header = { 320'h000031807e71441b141fe951b2b0c7dfc791d4646240fc2a2d1b80900020a24dc501ef1599fc48ed,
              320'h6cbac920af75575618e7b1e8eaf0b62a90d1942ea64d250357e9a09c063a47827c57b44e01000000};

   header2 = {320'h010000008fc749ba1129b477844abee559079e4ed02cf9eab69e763de5020bd15e09ca80f27b45c3,
              320'h3766594918ec67bd3a096dcc3a63fc015ce79821794e9fffa15743c5aafc944ef0ff0f1ed75e0000};

   testnum  =  26;
   $display(" Test %2d:  Load a Scrypt Test",testnum);
   for (i=0; i<20; i=i+1) begin
      address = HEADERbase + i*4;
      TileAccess({`UNIWR, 8'h00, myID, 8'h00, header[32*i +:32], address});
`ifdef   SHOW_HEADER
      $display("           Writing header   %h to address %h", header[32*i +:32], address );
`endif //SHOW_HEADER
      end

      repeat (10) @(posedge xclock);
   TileAccess({`UNIWR, 8'h00, myID, 8'h00, 32'h00000000, lock        });
   TileAccess({`UNIWR, 8'h00, myID, 8'h00, 32'h00000001, increment   });
   TileAccess({`UNIWR, 8'h00, myID, 8'h00, 32'h000003ff, ScryptTarget});
   TileAccess({`UNIWR, 8'h00, myID, 8'h00, 32'h00010000, nonce_end   });
   TileAccess({`UNIWR, 8'h00, myID, 8'h00, 32'h00010100, golden      });
   repeat (10) @(posedge xclock) `tCQ;

   testnum  =  27;
   $display(" Test %2d:  Let 'er rip again!! ",testnum);

   $display("            Release A2S ");

   TileAccess({`BCTRL, 8'h00, 8'h00, 8'h00, 32'h00000000, `aTILE_A2S_RESET});

   repeat (10) @(posedge xclock) `tCQ;
   testnum  =  28;
   $display(" Test %2d:  Verify Scrypt Completion",testnum);

   while (!xmiso[96] || !xmiso[95]) @(posedge xclock) `tCQ;
   xmiso_fb =  1'b1;
   if (xmiso[94]) $display("           Error4:  Found an error response");
   else $display("\n              INCOMING REQUEST RECEIVED \n");
   repeat ( 1) @(posedge xclock) `tCQ;
   xmiso_fb =  1'b0;
   $display("              COMMAND = %h", xmiso[95:64]);
   $display("              RD_DATA = %h", xmiso[63:32]);
   $display("              ADDRESS = %h", xmiso[31:0 ]);

   if (xmiso[63:32] == 32'h0000318f) $display("\n              RD_DATA %h is a Golden Nonce\n\n",xmiso[63:32]);

   $display("           Send an Acknowledge");
   TileAccess({`UNIWK, 8'h00, myID, 8'h00, 32'h600dbeef, 32'h0 });

   $display(" Test %2d:  Completed \n\n",testnum);

   $stop;

`endif   //HIGH_SPEED_TEST

   // END OF TESTS --------------------------------------------------------------------------------------------------------------
   $display("\n DONE \n");
   $stop;
   $finish;
   end

// TASKS =======================================================================================================================

task runSHA256;
   for (i=0; i<16; i=i+1)
      TileAccess({`UNIWR, 8'h00, myID, 8'h00, mesg[32*(15-i)+:32],   `aTILE_SHA256_MESG}); // Note indexing direction!
   for (i=0; i<8; i=i+1)
      TileAccess({`UNIWR, 8'h00, myID, 8'h00, hash[32*( 7-i)+:32],   `aTILE_SHA256_HASH});
   TileAccess   ({`UNIWR, 8'h00, myID, 8'h00, 32'h00000000,          `aTILE_SHA256_STRT_RDY});
   while (result == 0)
      TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h0,                 `aTILE_SHA256_STRT_RDY});
   for (i=0; i<8; i=i+1)   begin
      expect = xpct[32*(7-i)+:32];
      TileAccess({`UNIRD, 8'h00, myID, 8'h00, 32'h0,                 `aTILE_SHA256_HASH});
      if  (result !== expect)
         $display(" Error : Digest read       -- Expected %h  Found %h",expect,result);
      end
   $display("           SHA256 Test Complete");
   endtask

//===============================================================================================================================


task TileAccess;
input [95:0]   access;
   stimulus = access;
   start    = 1'b1;
   @(posedge xclock);
   start    = 1'b0;
   while (XFSM!=SEND) @(posedge xclock);
   @(posedge xclock);
   while (XFSM!=IDLE) @(posedge xclock);
endtask

//task TileAccess;
//   input [95:0]   access;
//   integer        wait4xirdy, wait4xordy;
//   reg            active;
//   integer        count;
//   if (access[95]) begin
//      count  = 0;
//      active = 1;
//      if (debug)                 $display("           Sending Request  %h",access[95:0]);
//      // Send the access out on xmosi and check it is taken by monitoring xmosi_fb (xirdy)
//      wait4xirdy  =  0;
//      wait4xordy  =  0;
//      XPUSH       =  1'b1;
//      while (!xmosi_fb && (wait4xirdy < 100))  begin
//         wait4xirdy = wait4xirdy +1;                        // keep a count of how long it takes
//         @(posedge xclock) `tCQ;                            // check xirdy is true else wait
//         count = count +1;
//         end
//      @(posedge xclock) `tCQ;                               // If true wait a cycle and ..
//      count = count +1;
//      XPUSH       =  1'b0;                                  // .. remove the push
//
//      if (wait4xirdy >= 100)     $display("           Not Ready!  Timed-out");
//      else if (access[95] && !access[93] && wait4xirdy < 100) begin   // If request and NOT a Broadcast wait for a response
//         while (!xmiso[`XORDY] && (wait4xordy < 100)) begin
//            wait4xordy = wait4xordy +1;                     // Wait for a response output ready
//            @(posedge xclock) `tCQ;
//            count = count +1;
//            end
//         @(posedge xclock) `tCQ;
//         count = count +1;
//      if (wait4xordy >= 100)  $display("           No Response:  Timed-out");
//         else begin
//            if (xmiso[94])    $display("           Error:  Found an error response");
//            if (debug) $display          ("           Response was     %h",xmiso[95:0]);
//            xmiso_fb =  1'b1;                                  // Send the feedback to pop the response
//            while ( xmiso[`XORDY]) begin
//               @(posedge xclock) `tCQ;
//               count = count +1;
//               end
//            @(posedge xclock) `tCQ;
//            count = count +1;
//            xmiso_fb =  1'b0;                                  // Release the pop
//            @(posedge xclock) `tCQ;
//            count = count +1;
//            end
//         end
//      active = 0;
//      end
//   else begin
//      count = 0;
//      active = 1;
//      if (debug)                 $display("           Sending Acknowledge Request",access[95:0]);
//      wait4xirdy  =  0;
//      wait4xordy  =  0;
//      XPUSH       =  1'b1;
//      while (!xmosi_fb && (wait4xirdy < 100))  begin
//         wait4xirdy = wait4xirdy +1;                        // keep a count of how long it takes
//         @(posedge xclock) `tCQ;                            // check xirdy is true else wait
//         count = count +1;
//         end
//      @(posedge xclock) `tCQ;                               // If true wait a cycle and ..
//      count = count +1;
//      XPUSH       =  1'b0;                                  // .. remove the push
//      active = 0;
//      end
//   @(posedge xclock) `tCQ;                               // If true wait a cycle and ..
//   endtask
////      else begin
////         if (debug)                 $display("           Sending Response %h",access[95:0]);
////         // Send the access out on xmosi and check it is taken by monitoring xmosi_fb (xirdy)
////         wait4xirdy  =  0;
////         wait4xordy  =  0;
////         XPUSH       =  1'b1;
////         while (!xmosi_fb && (wait4xirdy < 100))  begin
////            wait4xirdy = wait4xirdy +1;                        // keep a count of how long it takes
////            @(posedge xclock) `tCQ;                            // check xirdy is true else wait
////            end
////         @(posedge xclock) `tCQ;                               // If true wait a cycle and ..
////         XPUSH    =  1'b0;                                     // .. remove the push
////         if (wait4xirdy >= 100)     $display("           Not Ready!  Timed-out");
////       else begin
////          while (!xmiso[`XORDY] && (wait4xordy < 100)) begin
////             wait4xordy = wait4xordy +1;                     // Wait for a response output ready
////             @(posedge xclock) `tCQ;
////             end
////          @(posedge xclock) `tCQ;
////          if (wait4xordy >= 100)  $display("           No Response: Timed-out");
////          else begin
////             if (xmiso[94])    $display("           Error:  Response was unsolicited");
////             if (debug)        $display("           Response was     %h",xmiso[95:0]);
////             xmiso_fb =  1'b1;                                  // Send the feedback to pop the response
////             while ( xmiso[`XORDY]) @(posedge xclock) `tCQ;
////             xmiso_fb =  1'b0;                                  // Release the pop
////             @(posedge xclock) `tCQ;
////             end
////          end

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////


//module TileAccess (
//   // Output System Interconnect
//   input  wire          xmiso_clk,     // 97       96     95:88     87:80     79:72     71:64     63:32      31:0
//   input  wire [97:0]   xmiso,         // xtmo,    xordy, xack[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xrd0[31:0],xrd1[31:0]}
//   output reg           xmiso_fb,      //          xpop
//   // Input System Interconnect
//   output reg           xmosi_clk,     // 97       96     95:88     87:80     79:72     71:64     63:32      31:0
//   output reg  [97:0]   xmosi,         // xresetn, xpush, xcmd[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xwrd[31:0],xadr[31:0]}
//   input  wire          xmosi_fb,      //          xirdy
//   // Input from testbench                                95:88     87:80     79:72     71:64     63:32      31:0
//   input  wire [95:0]   stimulus,      //                 xcmd[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xwrd[31:0],xadr[31:0]}
//   input  wire          start,
//   // Output to testbench
//   output wire [95:0]   response,
//   output wire          finish,
//   // Global
//   input  wire          xclock,
//   input  wire          xresetn
//   );

// TILE ACCESS STATEMACHINE


endmodule
`endif   //verify_tile_one
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
