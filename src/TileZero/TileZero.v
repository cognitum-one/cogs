/*==============================================================================================================================

    Copyright © 2022..2025 Advanced Architectures

    All rights reserved
    Confidential Information
    Limited Distribution to Authorized Persons Only
    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

    Project Name         : Scrypt ASIC

    Description          : Control and Security Tile (Tile Zero)

================================================================================================================================
    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
================================================================================================================================
    Notes for Use and Synthesis

    The only true Master in the system is TileZero.  However, we view the Raceway as the Master and the Tiles as Slaves.
    Hence a Tile receives input over "xmosi" and responds over "xmiso"

    Be careful when a Tile is accessing another tile as it will send it's request over "xmiso" and expect a response over "xmosi"

===============================================================================================================================*/

`ifdef  synthesis //JLSW
   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh" //JLPL - 9_2_25
   //`include "A2_project_settings.vh" //JLPL - 9_2_25
`else
   `include "A2_project_settings.vh"
`endif

   `define  TZ_NEWS_DIV_DEFAULT  32'h00000000  //***+++ Not linking properly to A2_project_settings.vh ?
   `define  TZ_NEWS_CLK_DEFAULT  16'h0000 //***+++
   `define  TZ_RACE_CLK_DEFAULT  16'h0000 //***+++
   `define  TZ_TILE_CLK_DEFAULT  16'h0000 //***+++
   
   //`define  TEST_PUF_OSCILLATORS  //JLPL - 10_1_25
   //`define  SLOW_SYSTEM_PUF_OSC_TEST //JLPL - 10_5_25
   //`define  FAST_EMULATION_PUF_OSC_TEST //JLPL - 10_5_25


module TileZero #(
   parameter   VROMSIZE =  65536,                  // Bytes
   parameter   CODESIZE =  65536,                  // Bytes
   parameter   DATASIZE =  65536,                  // Bytes
   parameter   WORKSIZE =  16384,                  // Bytes
   parameter   RESET_DELAY = 130                   // Time Units (picoseconds)
   )(
`ifdef SIMULATE_CLOCK_RESET //JLSW
   input wire clockd, 
   input wire clocki, 
   input wire clocki_fast, 
   input wire reset,
   output wire [32:0]   imiso,                        // I/O interface to A2S //JLPL
   input wire  [47:0]   imosi,                        // I/O interface from A2S //JLPL
   output wire intp_news,                             //News Interrupt //JLPL
   input wire tz_clk,     //TileZero Raceway Input Clock //JLPL
   input wire [97:0] tz_data, //TileZero Raceway Input Data //JLPL
   output  wire tz_fb,      //TileZero Raceway Input Data Ready //JLPL
   output wire rz_clk,     //JLPL
   output wire [ 97:0]  rz_data,          // Raceway to TileZero xmosi //JLPL
   input  wire rz_fb, //JLPL
`endif //JLSW - endif SIMULATE_CLOCK_RESET
   // Output System Interconnect
   output wire          xmiso_clk,     // 97       96     95:88     87:80     79:72     71:64     63:32      31:0
   output wire [97:0]   xmiso,         // xtmo,    xordy, xack[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xrd0[31:0],xrd1[31:0]}
   input  wire          xmiso_fb,      //          xpop
   // Input System Interconnect
   input  wire          xmosi_clk,     // 97       96     95:88     87:80     79:72     71:64     63:32      31:0
   input  wire [97:0]   xmosi,         // xresetn, xpush,
   output wire          xmosi_fb,      //          xirdy
   // NEWS Interfaces
   output wire [ 8:0]   nmosi_data, emosi_data, wmosi_data, smosi_data,    // NEWS Output Pipeline Outputs //JL per Roger's interface name change in Cognitum_core
   output wire          nmosi_give, emosi_give, wmosi_give, smosi_give,    // NEWS Output Pipeline give
   input  wire          nmosi_take, emosi_take, wmosi_take, smosi_take,    // NEWS Output Pipeline take
   output wire          nmosi_clock,emosi_clock,wmosi_clock,smosi_clock,   // NEWS Output Pipeline Clock
   output wire          nmosi_reset,emosi_reset,wmosi_reset,smosi_reset,   // NEWS Output Pipeline Reset
   output wire          nmosi_clki, emosi_clki, wmosi_clki, smosi_clki,    // Individual clocks for serial interfaces -- direct to DFEs (clocki)

   input  wire [ 8:0]   nmiso_data, emiso_data, wmiso_data, smiso_data,    // NEWS Input  Pipeline Inputs //JL per Roger's interface name change in Cognitum_core
   input  wire          nmiso_give, emiso_give, wmiso_give, smiso_give,    // NEWS Input  Pipeline give
   output wire          nmiso_take, emiso_take, wmiso_take, smiso_take,    // NEWS Input  Pipeline take
   input  wire          nmiso_clock,emiso_clock,wmiso_clock,smiso_clock,   // NEWS Input  Pipeline Clock
   input  wire          nmiso_reset,emiso_reset,wmiso_reset,smiso_reset,   // NEWS Input  Pipeline Reset
   // PVT Interface
   //                                                                   15    14:10      9:4        3   2       1:0
   output wire [15:0]   pmosi,                     // PVT inputs  16 = {CLK,  TRIMG[4:0],TRIMO[5:0],ENA,VSAMPLE,PSAMPLE[1:0],}
   input  wire [10:0]   pmiso,                     // PVT outputs 11 = {data_valid, data[9:0] }
   // JTAG
   output wire          tdo,                       // JTAG
   input  wire          tdi,                       //***+++
   input  wire          tck,                       //
   input  wire          tms,                       //
   input  wire          trst,                      //*** Change - trst not used but bring in for dummy connection
   // Global Interconnect
   input  wire          sync_phase,                //JLPL - 10_23_25
   output wire          race_clock,                // Clock to North Hub of Raceway
   output wire          race_reset,                // Reset to North Hub of Raceway
   output wire [255:0]  flag_out,                  // To   Tile[N] //JL
   input  wire [255:0]  flag_in,                   // From Tile[N] //JL
   input  wire [255:1]  timo_in,                   // Timeout from each Tile //JLPL - 11_11_25
   input  wire [  3:0]  puf_osc,                   // Selected 4 of distributed oscillators for PUF
   input  wire          PoR_bypass,                // Power-On Reset bypass
   input  wire          PoR_extern,                // Power-On Reset external when bypassed
   input  wire          PoR                        // Power-On Reset
   );

`ifdef  synthesis //JLSW
   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/COGNITUM_IO_addresses.vh" //JLPL - 9_2_25
   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/COGNITUM_defines.vh" //JLPL - 9_2_25
//`include "COGNITUM_IO_addresses.vh" //JLPL - 9_2_25
//`include "COGNITUM_defines.vh" //JLPL - 9_2_25
`else
`include "COGNITUM_IO_addresses.vh"
`include "COGNITUM_defines.vh"
`endif

// Buses ------------------------------------------------------------------------------------------------------------------------
//    ae    AES
//    ci    CDC in
//    ck    clocks
//    cm    Code Memory
//    co    CDC out
//    dm    Data Memory
//    ep    NP WP SP             East North West South Pipelines
//    gf    GCM   Galois Field Multiplier Counter Mode
//    gw    Gateway
//    im    Internal Memory  -   Extra Code or Data for A2S
//    nn    NEWS  Network CoP
//    op    Output Pipe          to xmiso
//    rp    ROPUF
//    tr    TRNG
//    vr    Via ROM
//    xm    External Memory  -   Extra port for xmosi/xmiso and NEWS
//
// Cross reference:
//    amosi       incoming from raceway      from ci           goes to:  xm  gw
//    cmosi       Code accesses              from a2s          goes to:  vr  cm  im
//    dmosi       Data accesses              from a2s          goes to:  dm  im  xm
//    emosi       East NEWS interface        from nn           goes to:  ep
//    imosi       I/O  accesses              from a2s          goes to:  tr  gw  gf  ae  nn  ck  rp
//    kmosi       Key  accesses              from rp           goes to:  ae
//    lmosi       Local traffic              from nn           goes to:  xm
//    nmosi       North NEWS interface       from nn           goes to:  np
//    smosi       South NEWS interface       from nn           goes to:  sp
//    wmosi       West  NEWS interface       from nn           goes to:  wp
//    ymosi_gf    encrypted Block requests   from gf           goes to:  ae
//    ymosi_nn    encrypted Block requests   from nn           goes to:  ae
//
//    amiso_xm    access response            from xm           goes to:  op
//    amiso_gw    access request             from gw           goes to:  op
//    amiso       output to raceway          from op           goes to:  co
//    cmiso_ro    Code output                from vrM          goes to:  a2s
//    cmiso_cm    Code output                from cm           goes to:  a2s
//    cmiso_im    Code output                from im           goes to:  a2s
//    dmiso_cm    DATA output                from cm           goes to:  a2s
//    dmiso_dm    DATA output                from dm           goes to:  a2s
//    dmiso_im    DATA output                from im           goes to:  a2s
//    dmiso_xm    DATA output                from xm           goes to:  a2s
//    emiso       East NEWS interface        from ep           goes to:  np
//    imiso_ck    Register output            from ck           goes to:  a2s
//    imiso_rpuf  Register output            from rp           goes to:  a2s
//    imiso_gw    Register output            from gw           goes to:  a2s
//    imiso_gcm   Register output            from gf           goes to:  a2s
//    imiso_aes   Register output            from aes          goes to:  a2s
//    imiso_nn    Register output            from nn           goes to:  a2s
//    nmiso       North NEWS interface       from np           goes to:  nn
//    smiso       South NEWS interface       from sp           goes to:  nn
//    wmiso       West  NEWS interface       from wp           goes to:  nn
//    ymiso       encryption Block output    from ae           goes to:  gf   nn

// Raceway Interface

// A2S buses

wire xmosi_fb_rz; //JLPL - 9_16_25
reg   [31:0]   SCRATCH; //JLPL - 9_16_25

//wire  [`DMOSI_WIDTH-1:0]   cmosi;   // A2S Code //JLPL
wire  [             32:0]  cmosi; //JLPL
wire  [`DMISO_WIDTH-1:0]   cmiso,   cmiso_ro, cmiso_cm, cmiso_dm, cmiso_im;

wire  [66:0]               dmosi;   // A2S Data //JLPL - 9_16_25
wire  [`DMISO_WIDTH-1:0]   dmiso,   dmiso_cm, dmiso_dm, dmiso_im, dmiso_xm, dmiso_wm; //***+++ //JLPL - 8_28_25 - Add dmiso_wm
wire [32:0] imiso_scratch; //JLPL - 9_15_25

`ifdef SIMULATE_CLOCK_RESET //JLSW
wire  [32:0]   imiso_aes,  imiso_flag, imiso_gcm,  imiso_jtag, imiso_news, imiso_clks, //JLPL - 9_15_25
                      imiso_pvt,  imiso_rpuf, imiso_rw,   imiso_tmr0, imiso_tmr1;
`else //JLPL
wire  [47:0]   imosi;   // A2S IO //JLPL - 9_15_25
wire  [32:0]   imiso, imiso_aes,  imiso_flag, imiso_gcm,  imiso_jtag, imiso_news, imiso_clks, //JLPL - 9_15_25
                      imiso_pvt,  imiso_rpuf, imiso_rw,   imiso_tmr0, imiso_tmr1;
`endif //JLPL

wire           intpt, intpt_aes, intpt_fini, intpt_ldma, intpt_news, intpt_flag, //JLPL - 9_23_25 - Remove gcm interrupt
                      intpt_pvt, intpt_rpuf, intpt_tmr0, intpt_tmr1, intpt_timo;

//wire  [`KMOSI_WIDTH-1:0]   kmosi;   // RoPuf Keys to AES //*** Defines Missing so hardcode for compilation for now
wire  [47:0]   kmosi;   // RoPuf Keys to AES //***

//wire  [`LMOSI_WIDTH-1:0]   lmosi;   // Local DMA //*** Defines Missing so hardcode for compilation for now
//wire  [`LMISO_WIDTH-1:0]   lmiso;
wire  [66:0]   lmosi;   // Local DMA //*** //JLPL - 8_27_25
wire  [33:0]   lmiso;   //***

wire timo_zero; //JLPL - 11_12_25
wire [32:0] imiso_timo; //JLPL - 11_12_25


//wire  [`NMOSI_WIDTH-1:0]   nmosi;   // North NEWS //*** Defines Missing but already declared at Module Interface
//wire  [`NMISO_WIDTH-1:0]   nmiso;
//wire  [`NMOSI_WIDTH-1:0]   emosi;   // East  NEWS
//wire  [`NMISO_WIDTH-1:0]   emiso;
//wire  [`NMOSI_WIDTH-1:0]   smosi;   // South NEWS
//wire  [`NMISO_WIDTH-1:0]   smiso;
//wire  [`NMOSI_WIDTH-1:0]   wmosi;   // West  NEWS
//wire  [`NMISO_WIDTH-1:0]   wmiso;

//wire  [`YMOSI_WIDTH-1:0]   ymosi_gf, ymosi_nn; //*** Defines Missing so hardcode for compilation for now
//wire  [`YMISO_WIDTH-1:0]   ymiso; //***
//wire  [4:0]   ymosi_gf, ymosi_nn; //JLPL - 10_17_25
wire  [4:0]   ymosi_nn; //JLPL - 10_17_25
wire  [5:0]   ymosi_gf; //JLPL - 10_17_25
wire  [40:0]   ymiso; //JLSW
//wire ymiso_fb_gcm; //JLSW //JLPL - 9_15_25
wire ymiso_fb_news; //JLSW
wire gcm_parity_error; //JLPL - 11_8_25
wire news_parity_error; //JLPL - 11_8_25

// Nets -------------------------------------------------------------------------------------------------------------------------

// Clocks
wire                       ref_clock, zero_clock; //JLSW
// Synchronized resets
wire                       zero_reset; //JLPL - 9_16_25

// Memories
// A2S Processor

wire intpt_jtgi,intpt_jtgo; //JLPL - 11_4_25

//`ifdef  A2S_ENABLE_CONDITION_CODES   //JLPL - 10_23_25
wire  [ 17:1]  cond; //JLPL - 9_10_25 //JLPL - 10_23_25 - Change to 13 from 10 to add GCM signals //JLPL - 11_4_25 Change to 15 to add JTAG Signals //JLPL - 11_8_25 Change to 17 to add parity errors
wire  [ 1:0]   condc_gcm;  //JLPL - 10_23_25
//`endif //JLPL - 10_23_25
wire                       A2Son;

wire           ymiso_gf_ir; //JLPL - 9_15_25
wire tile_break = 1'b0; //JLSW - temporary

// System Interconnect===========================================================================================================
// Accesses arriving at a tile are already decoded as to tile address (xdst==myID) and so may be used directly based on

`ifdef SIMULATE_CLOCK_RESET //JLPL
assign   xmosi_fb    =  xmosi_fb_rz; //JLPL - 9_16_25
assign   rz_data     =  xmosi; //JLPL - 9_16_25
assign   rz_clk      =  xmosi_clk; //JLPL - 9_16_25
`else
assign   xmosi_fb    =  xmosi_fb_rz; //JLPL - 9_16_25
`endif

   
`ifdef SIMULATE_CLOCK_RESET //JLPL
wire [97:0] xmiso_rz; //JLPL - 9_10_25
//assign xmiso[97:96] = tz_data[97:96]; //JLPL - 9_10_25
assign xmiso[97:96] = xmiso_rz[97:96]; //JLPL - 9_10_25
wire [47:0] imosi_proc;  // I/O interface from A2S //JLPL - 9_10_25
`else
wire [97:0] xmiso_rz; //JLPL - 10_13_25
wire [47:0] imosi_proc;  // I/O interface from A2S //JLPL - 10_13_25
assign xmiso[97]    =  xmiso_rz[97]; //JLPL - 9_16_25
assign xmiso[96]    =  xmiso_rz[96]; //JLPL - 9_16_25
`endif   

// Resets =======================================================================================================================

wire zero_clock_boot;
wire zero_reset_boot;
wire done, boot, boot_reset;
wire  [10:0]   TCC; //JLPL - 11_7_25
wire  ring_oscillator_reset; //JLPL - 11_7_25

TileZero_boot i_TZB (
   .ref_clock  ( ref_clock),
   .div_clock  (zero_clock_boot), //JLPL - 9_2_25
   .ref_reset  (zero_reset_boot),
   .boot_reset (boot_reset),
   .boot       (boot),
   .ring_oscillator_reset  (ring_oscillator_reset), //JLPL - 11_7_25
   .TCC        (TCC),
   .PoR_bypass (PoR_bypass),                // Power-On Reset bypass
   .PoR_extern (PoR_extern),                // Power-On Reset external when bypassed
   .PoR        (PoR),                       // Power-On Reset
   .done       (done)
   );

// Clock generator & divider ====================================================================================================
// Clocks are required for Tile Zero,  Raceway, LVDS N,E,W & S  ( 6 in all )

wire race_clock_baseline; //JLPL

//JLPL - 9_2_25
TileZero_clocks #(
   .BASE       (ZCKS_base),
   .NUM_REGS   (ZCKS_range)
   ) i_CGZ (
   .TCC        (TCC),
   .north_clock(nmosi_clki),
   .south_clock(smosi_clki),
   .east_clock (emosi_clki),
   .west_clock (wmosi_clki),
   .race_clock (race_clock_baseline),
   .race_reset (race_reset    ),                            //JLPL - 9_14_25
   .ring_oscillator_reset  (ring_oscillator_reset), //JLPL - 11_7_25
   .imiso      (imiso_clks),
//   .imosi      (imosi_proc),                                //JLPL - 9_14_25 //JLPL - 10_22_25
`ifdef SIMULATE_CLOCK_RESET //JLPL
   .imosi   (imosi_proc),                    // I/O Register Master-out slave-in
`else
   .imosi   (imosi),                         // I/O Register Master-out slave-in
`endif //JLPL
   .enable     (1'b1),
   .reset      (zero_reset_boot),                           // Reset to force the oscillators to steady state //JLPL - 9_2_25
   .clock      (zero_clock_boot)                            // Reset to force the oscillators to steady state //JLPL - 9_2_25
   );
//End JLPL - 9_2_25
 
`ifdef SIMULATE_CLOCK_RESET //JLSW
    assign zero_clock = clockd;
	//assign race_clock = clockd; //JLPL - 9_14_25
	assign race_clock = race_clock_baseline; //JLPL - 9_14_25
`else
    assign zero_clock = zero_clock_boot; //JLPL - 9_8_25
	assign race_clock = race_clock_baseline; //JLPL
`endif //JLSW - endif SIMULATE_CLOCK_RESET
 

// Tile input Clock domain crossing ---------------------------------------------------------------------------------------------


// MEMORIES /////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

// BootROM ======================================================================================================================


`ifdef SIMULATE_CLOCK_RESET //JLSW
    //assign zero_reset = first_reset; //JLSW - temporary //JLPL - 9_4_25
    assign zero_reset = zero_reset_boot; //JLSW - temporary //JLPL - 9_4_25
	//assign race_reset = first_reset; //JLPL - 9_14_25
`else
assign   zero_reset = zero_reset_boot; //JLPL - 9_2_25
//assign   race_reset = first_reset; //JLPL - 9_14_25
`endif //JLSW - endif SIMULATE_CLOCK_RESET

// MEMORIES /////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

//JLPL - 9_2_25
code_mem i_CM (
   .cmiso      (cmiso_cm[33:0]),    // 34 = {grnt, ack,  rdd[31:0]}            -- grant, acknowledge, read-data
   .cmosi      (cmosi   [32:0]),    // 33 = {en,                    adr[31:0]} -- request,  address
   .dmiso      (dmiso_cm[33:0]),    // 34 = {grnt, ack,  rdd[31:0]}            -- grant, acknowledge, read-data
   .dmosi      (dmosi   [66:0]),    // 67 = {en, wr, rd, wdd[31:0], adr[31:0]} -- request, write, write-data address
   .done       (done          ),
   .boot       (boot          ),
   .reset      (boot_reset    ),    // reset before boot!!
   .clock      (zero_clock_boot)     //JLPL - 9_3_25
   );  
   
data_mem i_DM (
   .cmiso      (cmiso_dm[33:0]),    // 34 = {agnt, aack, ardd[31:0]}
   .cmosi      (cmosi   [32:0]),    // 33 = {areq,                       aadr[31:0]}  // read only
   .dmiso      (dmiso_dm[33:0]),    // 34 = {bgnt, back,     brdd[31:0]}
   .dmosi      (dmosi   [66:0]),    // 66 = {breq, bwr, brd, bwrd[31:0], badr[31:0]}
   .clock      (zero_clock_boot) //JLPL - 9_4_25
   );
//End JLPL - 9_2_25

//JLPL - 8_29_25


wire flag_all;
//wire xmosi_fb_rz; //JLPL - 9_16_25
wire xmiso_clk_rz;
//wire [97:0] xmiso_rz; //JLPL - 9_10_25
wire [7:0] po_ctrl; //JLPL - 9_22_25
wire [35:0] pufvector; //JLPL - 9_22_25 //JLPL - 9_30_25 -> Remove initialization
wire puf_enable; //JLPL - 9_22_25 //JLPL - 9_30_25 -> Remove initialization
wire PRO0,PRO1; //JLPL - 9_27_25
wire [3:0] tile0_puf_osc; //JLPL - 9_27_25
wire [3:0] final_puf_osc; //JLPL - 9_27_25

//JLPL - 9_27_25
// PUF OSCILLATORS & CONTROL ----------------------------------------------------------------------------------------------------
//                       7 6 5 4 3 2 1 0
//                      +-+-+---+-+-+---+    R: Run the oscillator
//                      |E|R| A |E|R| A |    E: Enable power to the oscillator
//                      +-+-+---+-+-+---+    A: Address of lane to put output onto
//
//PUF_OSC iPUFOa (.Y(PRO0), .RUN(po_ctrl[2]), .EN(po_ctrl[3]));
//PUF_OSC iPUFOb (.Y(PRO1), .RUN(po_ctrl[6]), .EN(po_ctrl[7]));

`ifdef synthesis
PufOsc_A9_C14 iPUFOa (.Y(PRO0), .RUN(po_ctrl[2]), .EN(po_ctrl[3]));
PufOsc_A9_C14 iPUFOb (.Y(PRO1), .RUN(po_ctrl[6]), .EN(po_ctrl[7]));
`else
PUF_OSC       iPUFOa (.Y(PRO0), .RUN(po_ctrl[2]), .EN(po_ctrl[3]));
PUF_OSC       iPUFOb (.Y(PRO1), .RUN(po_ctrl[6]), .EN(po_ctrl[7]));
`endif

// Assign to PO_OSC_LANES
assign   tile0_puf_osc[3:0]   = (PRO0 << po_ctrl[1:0]) | (PRO1 << po_ctrl[5:4]);
assign final_puf_osc = puf_osc | tile0_puf_osc;
//End JLPL - 9_27_25

RacewayZero i_RW(
   .xmiso_clk  (xmiso_clk_rz  ),    // Output to Raceway
   .xmiso      (xmiso_rz      ),
   .xmiso_fb   (xmiso_fb      ),
   .xmosi_clk  (xmosi_clk     ),    // Input from Raceway
   .xmosi      (xmosi         ),
   .xmosi_fb   (xmosi_fb_rz   ),
   .bmiso      (dmiso_wm[33:0]),    // 34 = {bgnt, back,     brdd[31:0]}                  // A2S  port
   .bmosi      (dmosi   [66:0]),    // 66 = {breq, bwr, brd, bwrd[31:0], badr[31:0]}
   .cmiso      (lmiso   [33:0]),    // 34 = {bgnt, back,     brdd[31:0]}                  // LDMA port
   .cmosi      (lmosi   [66:0]),    // 66 = {breq, bwr, brd, bwrd[31:0], badr[31:0]}
   .imiso      (imiso_rw[32:0]),    // 33 = {iack,           irdd[31:0]}
//   .imosi      (imosi_proc [47:0]),    // 48 = {ireq, icmd[1:0],iwrd[31:0], iadr[12:0] } //JLPL - 9_10_25 //JLPL - 10_22_25
`ifdef SIMULATE_CLOCK_RESET //JLPL
   .imosi   (imosi_proc),                    // I/O Register Master-out slave-in
`else
   .imosi   (imosi),                         // I/O Register Master-out slave-in
`endif //JLPL
   .intpt_fini (intpt_fini    ),    // Interrupts
   .intpt_timo (timo_zero     ),    //JLPL - 11_12_25
   .flag_out   (flag_all      ),
   .po_ctrl    (po_ctrl       ), //JLPL - 9_22_25
   .pufvector  (pufvector     ), //JLPL - 9_22_25
   .puf_enable (puf_enable    ), //JLPL - 9_22_25
   .reset      (zero_reset    ),
   .clock      (zero_clock_boot) //JLPL - 9_4_25
   );
//End JLPL - 8_29_25

// Gather Buses ================================================================================================================

assign   cmiso =  cmiso_cm | cmiso_dm;
assign   dmiso =  dmiso_cm | dmiso_dm | dmiso_wm; //JLPL - 8_28_25 - Swap dmiso_xm with dmiso_wm
assign   imiso = imiso_aes  | imiso_flag | imiso_gcm  | imiso_jtag | imiso_news | imiso_pvt  | imiso_rpuf | imiso_rw //JLPL - 9_15_25
         | imiso_tmr0 | imiso_tmr1 | imiso_clks | imiso_scratch | imiso_timo; //JLPL - 11_12_25

// A2Sv2 Processor ==============================================================================================================

assign                           //JLPL - 9_10_25
   intpt       = |cond[11:1],    //JLPL - 11_4_25
   cond[17:1]  = {gcm_parity_error, news_parity_error, sync_phase, condc_gcm[1:0], intpt_jtgi, intpt_jtgo, intpt_aes, intpt_fini, intpt_ldma, intpt_news, intpt_flag, //JLPL - 11_8_25
                                  intpt_pvt, intpt_rpuf, intpt_tmr1, intpt_tmr0, intpt_timo };  //JLPL - 10_23_25


`ifdef SIMULATE_CLOCK_RESET //JLPL
wire [32:0] imiso_proc;  // I/O interface to A2S
//wire [47:0] imosi_proc;  // I/O interface from A2S //JLPL - 9_10_25
reg [31:0] hold_io_value = 32'h0;
wire [12:0] imosi_add;
wire [31:0] imosi_data;
wire [2:0]  imosi_cmd;
reg miso_scratch_ack = 1'b0;
//wire [32:0] imiso_scratch; //JLPL - 9_15_25
reg [31:0] imiso_scratch_read;

assign imiso_scratch = {miso_scratch_ack,imiso_scratch_read}; //JLPL - 9_5_25

assign imosi_add = imosi_proc[12:0];
assign imosi_data = imosi_proc[44:13];
assign imosi_cmd = imosi_proc[47:45];

assign imiso_proc = imiso_scratch | imiso; //JLPL - 9_5_25
//assign imiso_proc[32] = miso_scratch_ack | imiso_lv[32]; //JLPL - 9_4_25
//assign imiso_proc[31:0] = hold_io_value;

always @(posedge zero_clock_boot)
	begin
	   if ( imosi_proc[47] && imosi_add == 13'h1043 )
		hold_io_value <= imosi_data;
	end
	
always @(posedge zero_clock_boot)
	begin
	   if ( imosi_proc[47] && imosi_add == 13'h1023 && imosi_cmd == 3'b110 ) //JLPL - 10_8_25 -> Cognitum_IO_Addresses change
		SCRATCH <= imosi_data;	
	end	
	
always @(*)
	begin
	   if ( imosi_proc[47] && imosi_add == 13'h1023 ) //JLPL - 10_8_25 -> Cognitum_IO_Addresses change
		miso_scratch_ack <= #0.3 1'b1;
       else
		miso_scratch_ack <= #0.3 1'b0;	
		
	   if ( imosi_proc[47] && imosi_add == 13'h1023 && imosi_cmd == 3'b101 ) //JLPL - 10_8_25 -> Cognitum_IO_Addresses change
		 imiso_scratch_read <= SCRATCH;	//JLPL - 9_15_25
	   else
	     imiso_scratch_read <= 32'h0;		
	end
`else

wire [12:0] imosi_add;
wire [31:0] imosi_data;
wire [2:0]  imosi_cmd;
reg miso_scratch_ack = 1'b0;

reg [31:0] imiso_scratch_read;

assign imiso_scratch = {miso_scratch_ack,imiso_scratch_read}; //JLPL - 9_5_25

assign imosi_add = imosi[12:0];
assign imosi_data = imosi[44:13];
assign imosi_cmd = imosi[47:45];
	
always @(posedge zero_clock_boot)
	begin
	   if ( imosi[47] && imosi_add == 13'h1023 && imosi_cmd == 3'b110 ) //JLPL - 10_8_25 -> Cognitum_IO_Addresses change
		SCRATCH <= imosi_data;	
	end	
	
always @(*)
	begin
	   if ( imosi[47] && imosi_add == 13'h1023 ) //JLPL - 10_8_25 -> Cognitum_IO_Addresses change
		miso_scratch_ack <= #0.3 1'b1;
       else
		miso_scratch_ack <= #0.3 1'b0;	
		
	   if ( imosi[47] && imosi_add == 13'h1023 && imosi_cmd == 3'b101 ) //JLPL - 10_8_25 -> Cognitum_IO_Addresses change
		 imiso_scratch_read <= SCRATCH;	//JLPL - 9_15_25
	   else
	     imiso_scratch_read <= 32'h0;		
	end	
`endif //JLPL

//`undef  A2S_ENABLE_FLOATING_POINT            // Not used in TileZero //JLPL - 7_16_25
//`undef  A2S_ENABLE_SIMD                      // Not used in TileZero //JLPL - 7_16_25
//A2Sv2r3 #(                             //JL Change to A2Sv2r3 from A2Sv2r2 temporarily //JLPL - 7_16_25 //JLPL - 9_4_25 Remove _Tz
//   .WA2S    (`A2S_MACHINE_WIDTH),
//   .WINS    (`A2S_INSTRUCTION_WIDTH),
//   .DRSK    (32),                            // A2S Depth of Return Stack
//   .DDSK    (32)                             // A2S Depth of Data Stack
//   ) i_A2S (
//   .dmosi   (dmosi),                         // Data Memory  Master-out slave-in
//   .cmosi   (cmosi),                         // Code Memory  Master-out slave-in
//`ifdef SIMULATE_CLOCK_RESET //JLPL
//   .imosi   (imosi_proc),                    // I/O Register Master-out slave-in
//`else
//   .imosi   (imosi),                         // I/O Register Master-out slave-in
//`endif //JLPL
//   .dmiso   (dmiso),                         // Data Memory  Master-in slave-out
//   .cmiso   (cmiso),                         // Code Memory  Master-in slave-out
//`ifdef SIMULATE_CLOCK_RESET //JLPL
//   .imiso   (imiso_proc),                    // I/O Register Master-in slave-out
//`else
//   .imiso   (imiso),                         // I/O Register Master-in slave-out
//`endif //JLPL
//   .intpt   (intpt),
//   .ccode   (cond),
//   .activ   (A2Son),
//   .reset   (zero_reset_boot), //JLPL - 9_3_25
//   .fstck   (zero_clock_boot),   //JL - temporarily match this processor to A2Sv2r3 ... instead of A2Sv2r2 //JLPL - 9_3_25
//   .clock   (zero_clock_boot) //JLPL - 9_3_25
//   ); //JLPL - 9_4_25

//`define  MODE_ZERO //JLPL - 10_23_25
A2Sv2r3_TZ i_A2S (               //JLPL - 10_23_25
   .dmosi   (dmosi),             // A2S to   Data Memory
   .dmiso   (dmiso),             // A2S from Data Memory
   .cmosi   (cmosi),             // A2S to   Code Memory
   .cmiso   (cmiso),             // A2S from Code Memory
`ifdef SIMULATE_CLOCK_RESET //JLPL
   .imosi   (imosi_proc),                    // I/O Register Master-out slave-in
`else
   .imosi   (imosi),                         // I/O Register Master-out slave-in
`endif //JLPL
`ifdef SIMULATE_CLOCK_RESET //JLPL
   .imiso   (imiso_proc),                    // I/O Register Master-in slave-out
`else
   .imiso   (imiso),                         // I/O Register Master-in slave-out
`endif //JLPL
   .intpt   (intpt),             // Interrupts
   .ccode   (cond ),             // Condition Codes
   .activ   (A2Son),
   .fstck   (1'b0 ),
   .reset   (zero_reset_boot),
   .clock   (zero_clock_boot)
   );
//`undef   MODE_ZERO //JLPL - 10_23_25

// I/O (Co-processors) ==========================================================================================================

// Gateway ----------------------------------------------------------------------------------------------------------------------

wire amosi_gw_fb; //***
wire dummy_flag_out; //***
wire [8:0] puf0_vector; //JLPL - 9_30_25
wire [8:0] puf1_vector; //JLPL - 9_30_25
wire [8:0] puf2_vector; //JLPL - 9_30_25
wire [8:0] puf3_vector; //JLPL - 9_30_25
wire [3:0] osc_out; //JLPL - 9_30_25
//assign flag_out = 256'h0; //JLPL - temporary missing drive source //JLPL - 9_7_25
assign pufvector = {puf3_vector,puf2_vector,puf1_vector,puf0_vector}; //JLPL - 9_30_25

//JLPL - 10_1_25 - 10_5_25
`ifdef FAST_EMULATION_PUF_OSC_TEST //JLPL - 10_1_25
wire [3:0] test_osc_out;
reg sync_puf_enable = 1'b0;
reg [4:0] count_to_20 = 5'h0;
reg [1:0] drop_counter = 2'h0;
wire [19:0] drop_clocks [7:0];
reg [3:0] block_clocks = 4'h0;
reg [2:0] random_delay_skip0_count = 3'h5; //JLPL - 10_5_25
reg [2:0] random_delay_skip2_count = 3'h2; //JLPL - 10_5_25
reg sync_puf_enable1 = 1'b0; //JLPL - 10_5_25
reg random_skip0 = 1'b0; //JLPL - 10_5_25
reg random_skip2 = 1'b0; //JLPL - 10_5_25
reg enable_puf_noise = 1'b0; //JLPL - 10_6_25 -> Disable as default and use testbench to force signal to enable noise profile

wire [2:0] puf0_divider = puf0_vector[2:0]; //JLPL - 10_4_25 - 8:6 no statistical variance counter0 vs counter1 pairs - Use 2:0 instead per Abdoulaye - bit 0 probably enough
wire [2:0] puf1_divider = puf1_vector[2:0]; //JLPL - 10_4_25 - 8:6 no statistical variance counter0 vs counter1 pairs - Use 2:0 instead per Abdoulaye - bit 0 probably enough
wire [2:0] puf2_divider = puf2_vector[2:0]; //JLPL - 10_4_25 - 8:6 no statistical variance counter0 vs counter1 pairs - Use 2:0 instead per Abdoulaye - bit 0 probably enough
wire [2:0] puf3_divider = puf3_vector[2:0]; //JLPL - 10_4_25 - 8:6 no statistical variance counter0 vs counter1 pairs - Use 2:0 instead per Abdoulaye - bit 0 probably enough

assign drop_clocks[0] = {5'h18,5'h18,5'h18,5'h18}; //0,0,0,0 drops
assign drop_clocks[1] = {5'h12,5'h12,5'h12,5'h12}; //1,1,1,1 drops -> ~ 12 count drop from previous frequency (i.e. - index 0)
assign drop_clocks[2] = {5'h12,5'h11,5'h11,5'h11}; //1,2,2,2 drops -> ~ 9 count drop from previous frequency  (i.e. - index 1)
assign drop_clocks[3] = {5'h11,5'h10,5'h11,5'h10}; //2,3,2,3 drops -> ~ 9 count drop from previous frequency  (i.e. - index 2)
assign drop_clocks[4] = {5'h10,5'h10,5'h10,5'h0F}; //3,3,3,4 drops -> ~ 9 count drop from previous frequency  (i.e. - index 3)
assign drop_clocks[5] = {5'h0F,5'h0F,5'h0F,5'h0F}; //4,4,4,4 drops -> ~ 9 count drop from previous frequency  (i.e. - index 4)
assign drop_clocks[6] = {5'h0E,5'h0F,5'h0E,5'h0E}; //5,4,5,5 drops -> ~ 9 count drop from previous frequency  (i.e. - index 5)
assign drop_clocks[7] = {5'h0E,5'h0D,5'h0E,5'h0D}; //5,6,5,6 drops -> ~ 9 count drop from previous frequency  (i.e. - index 6)

assign test_osc_out[3] = zero_clock_boot & !block_clocks[3] & sync_puf_enable;
assign test_osc_out[2] = zero_clock_boot & !block_clocks[2] & sync_puf_enable & !random_skip2; //JLPL - 10_6_25
assign test_osc_out[1] = zero_clock_boot & !block_clocks[1] & sync_puf_enable;
assign test_osc_out[0] = zero_clock_boot & !block_clocks[0] & sync_puf_enable & !random_skip0; //JLPL - 10_6_25

wire [4:0] test = drop_clocks[0][4:0];

always @(negedge zero_clock_boot)
	begin
	    sync_puf_enable1 <= sync_puf_enable;
		sync_puf_enable <= puf_enable;

		if ( !sync_puf_enable )
			begin
				count_to_20  <= 5'h0;
				drop_counter <= 2'h0;
				block_clocks <= 4'h0;
				random_skip0 <= enable_puf_noise; //JLPL - 10_6_25
				random_skip2 <= enable_puf_noise; //JLPL - 10_6_25
			end
		else
			begin
			    if ( !sync_puf_enable1 && sync_puf_enable ) //JLPL - 10_5_25
					begin //JLPL - 10_5_25
						random_delay_skip0_count <= random_delay_skip0_count + puf0_vector[2:0]; //JLPL - 10_5_25
						random_delay_skip2_count <= random_delay_skip2_count + puf2_vector[2:0]; //JLPL - 10_5_25					
					end //JLPL - 10_5_25
				if ( random_skip0 && count_to_20[3:0] >= {1'b0,random_delay_skip0_count} ) //JLPL - 10_5_25
					random_skip0 <= 1'b0; //JLPL - 10_5_25
				if ( random_skip2 && count_to_20[3:0] >= {1'b0,random_delay_skip2_count} ) //JLPL - 10_5_25
					random_skip2 <= 1'b0; //JLPL - 10_5_25					
				if ( count_to_20 == 5'h13 )
					begin
						count_to_20 <= 5'h0;
						drop_counter <= drop_counter + 1;
						block_clocks <= 4'h0;
					end
				else
					begin
					   count_to_20 <= count_to_20 + 1;
					   if ( drop_counter == 2'b00 )
						begin
							if ( count_to_20 == drop_clocks[puf0_divider][4:0] )
								block_clocks[0] <= 1'b1;
							if ( count_to_20 == drop_clocks[puf1_divider][4:0] )
								block_clocks[1] <= 1'b1;
							if ( count_to_20 == drop_clocks[puf2_divider][4:0] )
								block_clocks[2] <= 1'b1;
							if ( count_to_20 == drop_clocks[puf3_divider][4:0] )
								block_clocks[3] <= 1'b1;
						end //end of if drop_counter = 00
					   if ( drop_counter == 2'b01 )
						begin
							if ( count_to_20 == drop_clocks[puf0_divider][9:5] )
								block_clocks[0] <= 1'b1;
							if ( count_to_20 == drop_clocks[puf1_divider][9:5] )
								block_clocks[1] <= 1'b1;
							if ( count_to_20 == drop_clocks[puf2_divider][9:5] )
								block_clocks[2] <= 1'b1;
							if ( count_to_20 == drop_clocks[puf3_divider][9:5] )
								block_clocks[3] <= 1'b1;
						end //end of if drop_counter = 01
					   if ( drop_counter == 2'b10 )
						begin
							if ( count_to_20 == drop_clocks[puf0_divider][14:10] )
								block_clocks[0] <= 1'b1;
							if ( count_to_20 == drop_clocks[puf1_divider][14:10] )
								block_clocks[1] <= 1'b1;
							if ( count_to_20 == drop_clocks[puf2_divider][14:10] )
								block_clocks[2] <= 1'b1;
							if ( count_to_20 == drop_clocks[puf3_divider][14:10] )
								block_clocks[3] <= 1'b1;
						end //end of if drop_counter = 10
					   if ( drop_counter == 2'b11 )
						begin
							if ( count_to_20 == drop_clocks[puf0_divider][19:15] )
								block_clocks[0] <= 1'b1;
							if ( count_to_20 == drop_clocks[puf1_divider][19:15] )
								block_clocks[1] <= 1'b1;
							if ( count_to_20 == drop_clocks[puf2_divider][19:15] )
								block_clocks[2] <= 1'b1;
							if ( count_to_20 == drop_clocks[puf3_divider][19:15] )
								block_clocks[3] <= 1'b1;
						end //end of if drop_counter = 11
					end
			end //end else sync_puf_enable

	end
`endif //endif FAST_EMULATION_PUF_OSC_TEST
//End JLPL - 10_1_25 - 10_5_25


//JLPL - 10_1_25
`ifdef SLOW_SYSTEM_PUF_OSC_TEST //JLPL - 10_1_25 - 10_5_25
wire [3:0] test_osc_out;
reg sync_puf_enable;
reg [4:0] count3_to_20 = 5'h0; //JLPL - 10_4_25
reg [1:0] drop3_counter = 2'h0; //JLPL - 10_4_25
reg [4:0] count2_to_20 = 5'h0; //JLPL - 10_4_25
reg [1:0] drop2_counter = 2'h0; //JLPL - 10_4_25
reg [4:0] count1_to_20 = 5'h0; //JLPL - 10_4_25
reg [1:0] drop1_counter = 2'h0; //JLPL - 10_4_25
reg [4:0] count0_to_20 = 5'h0; //JLPL - 10_4_25
reg [1:0] drop0_counter = 2'h0; //JLPL - 10_4_25
reg [5:0] puf_osc3_enable = 6'h0; //JLPL - 10_4_25
reg [5:0] puf_osc2_enable = 6'h0; //JLPL - 10_4_25
reg [5:0] puf_osc1_enable = 6'h0; //JLPL - 10_4_25
reg [5:0] puf_osc0_enable = 6'h0; //JLPL - 10_4_25
wire puf_osc_reset = puf_osc3_enable[5] & puf_osc2_enable[5] & puf_osc1_enable[5] & puf_osc0_enable[5];
wire [19:0] drop_clocks [7:0];
reg [3:0] block_clocks = 4'h0;

wire [2:0] puf0_divider = puf0_vector[2:0]; //JLPL - 10_4_25 - 8:6 no statistical variance counter0 vs counter1 pairs - Use 2:0 instead per Abdoulaye - bit 0 probably enough
wire [2:0] puf1_divider = puf1_vector[2:0]; //JLPL - 10_4_25 - 8:6 no statistical variance counter0 vs counter1 pairs - Use 2:0 instead per Abdoulaye - bit 0 probably enough
wire [2:0] puf2_divider = puf2_vector[2:0]; //JLPL - 10_4_25 - 8:6 no statistical variance counter0 vs counter1 pairs - Use 2:0 instead per Abdoulaye - bit 0 probably enough
wire [2:0] puf3_divider = puf3_vector[2:0]; //JLPL - 10_4_25 - 8:6 no statistical variance counter0 vs counter1 pairs - Use 2:0 instead per Abdoulaye - bit 0 probably enough

assign drop_clocks[0] = {5'h18,5'h18,5'h18,5'h18}; //0,0,0,0 drops
assign drop_clocks[1] = {5'h12,5'h12,5'h12,5'h12}; //1,1,1,1 drops -> ~ 12 count drop from previous frequency (i.e. - index 0)
assign drop_clocks[2] = {5'h12,5'h11,5'h11,5'h11}; //1,2,2,2 drops -> ~ 9 count drop from previous frequency  (i.e. - index 1)
assign drop_clocks[3] = {5'h11,5'h10,5'h11,5'h10}; //2,3,2,3 drops -> ~ 9 count drop from previous frequency  (i.e. - index 2)
assign drop_clocks[4] = {5'h10,5'h10,5'h10,5'h0F}; //3,3,3,4 drops -> ~ 9 count drop from previous frequency  (i.e. - index 3)
assign drop_clocks[5] = {5'h0F,5'h0F,5'h0F,5'h0F}; //4,4,4,4 drops -> ~ 9 count drop from previous frequency  (i.e. - index 4)
assign drop_clocks[6] = {5'h0E,5'h0F,5'h0E,5'h0E}; //5,4,5,5 drops -> ~ 9 count drop from previous frequency  (i.e. - index 5)
assign drop_clocks[7] = {5'h0E,5'h0D,5'h0E,5'h0D}; //5,6,5,6 drops -> ~ 9 count drop from previous frequency  (i.e. - index 6)

assign test_osc_out[3] = final_puf_osc[3] & !block_clocks[3] & puf_enable & puf_osc_reset;
assign test_osc_out[2] = final_puf_osc[2] & !block_clocks[2] & puf_enable & puf_osc_reset;
assign test_osc_out[1] = final_puf_osc[1] & !block_clocks[1] & puf_enable & puf_osc_reset;
assign test_osc_out[0] = final_puf_osc[0] & !block_clocks[0] & puf_enable & puf_osc_reset;

always @(negedge final_puf_osc[3] or negedge puf_enable )
	begin

		if ( !puf_enable )
			begin
				count3_to_20  <= 5'h0;
				drop3_counter <= 2'h0;
				block_clocks[3] <= 1'b0;
				puf_osc3_enable <= 5'h0;
			end
		else
			begin
				if ( count3_to_20 == 5'h13 )
					begin
						count3_to_20 <= 5'h0;
						drop3_counter <= drop3_counter + 1;
						block_clocks[3] <= 1'b0;
					end
				else
					begin
					   puf_osc3_enable <= {puf_osc3_enable[4:0],1'b1};
					   if ( puf_osc_reset )
						count3_to_20 <= count3_to_20 + 1;
					   if ( drop3_counter == 2'b00 )
						begin
							if ( count3_to_20 == drop_clocks[puf3_divider][4:0] )
								block_clocks[3] <= 1'b1;
						end //end of if drop_counter = 00
					   if ( drop3_counter == 2'b01 )
						begin
							if ( count3_to_20 == drop_clocks[puf3_divider][9:5] )
								block_clocks[3] <= 1'b1;
						end //end of if drop_counter = 01
					   if ( drop3_counter == 2'b10 )
						begin
							if ( count3_to_20 == drop_clocks[puf3_divider][14:10] )
								block_clocks[3] <= 1'b1;
						end //end of if drop_counter = 10
					   if ( drop3_counter == 2'b11 )
						begin
							if ( count3_to_20 == drop_clocks[puf3_divider][19:15] )
								block_clocks[3] <= 1'b1;
						end //end of if drop_counter = 11
					end
			end //end else puf_enable

	end //end always #3
	
always @(negedge final_puf_osc[2] or negedge puf_enable)
	begin

		if ( !puf_enable )
			begin
				count2_to_20  <= 5'h0;
				drop2_counter <= 2'h0;
				block_clocks[2] <= 1'b0;
				puf_osc2_enable <= 5'h0;
			end
		else
			begin
				if ( count2_to_20 == 5'h13 )
					begin
						count2_to_20 <= 5'h0;
						drop2_counter <= drop2_counter + 1;
						block_clocks[2] <= 1'b0;
					end
				else
					begin
					   puf_osc2_enable <= {puf_osc2_enable[4:0],1'b1};
					   if ( puf_osc_reset )
						count2_to_20 <= count2_to_20 + 1;
					   if ( drop2_counter == 2'b00 )
						begin
							if ( count2_to_20 == drop_clocks[puf2_divider][4:0] )
								block_clocks[2] <= 1'b1;
						end //end of if drop_counter = 00
					   if ( drop2_counter == 2'b01 )
						begin
							if ( count2_to_20 == drop_clocks[puf2_divider][9:5] )
								block_clocks[2] <= 1'b1;
						end //end of if drop_counter = 01
					   if ( drop2_counter == 2'b10 )
						begin
							if ( count2_to_20 == drop_clocks[puf2_divider][14:10] )
								block_clocks[2] <= 1'b1;
						end //end of if drop_counter = 10
					   if ( drop2_counter == 2'b11 )
						begin
							if ( count2_to_20 == drop_clocks[puf2_divider][19:15] )
								block_clocks[2] <= 1'b1;
						end //end of if drop_counter = 11
					end
			end //end else puf_enable

	end //end always #2	
	
always @(negedge final_puf_osc[1] or negedge puf_enable)
	begin

		if ( !puf_enable )
			begin
				count1_to_20  <= 5'h0;
				drop1_counter <= 2'h0;
				block_clocks[1] <= 1'b0;
				puf_osc1_enable <= 5'h0;
			end
		else
			begin
				if ( count1_to_20 == 5'h13 )
					begin
						count1_to_20 <= 5'h0;
						drop1_counter <= drop1_counter + 1;
						block_clocks[1] <= 1'b0;
					end
				else
					begin
					   puf_osc1_enable <= {puf_osc1_enable[4:0],1'b1};
					   if ( puf_osc_reset )
						count1_to_20 <= count1_to_20 + 1;
					   if ( drop1_counter == 2'b00 )
						begin
							if ( count1_to_20 == drop_clocks[puf1_divider][4:0] )
								block_clocks[1] <= 1'b1;
						end //end of if drop_counter = 00
					   if ( drop1_counter == 2'b01 )
						begin
							if ( count1_to_20 == drop_clocks[puf1_divider][9:5] )
								block_clocks[1] <= 1'b1;
						end //end of if drop_counter = 01
					   if ( drop1_counter == 2'b10 )
						begin
							if ( count1_to_20 == drop_clocks[puf1_divider][14:10] )
								block_clocks[1] <= 1'b1;
						end //end of if drop_counter = 10
					   if ( drop1_counter == 2'b11 )
						begin
							if ( count1_to_20 == drop_clocks[puf1_divider][19:15] )
								block_clocks[1] <= 1'b1;
						end //end of if drop_counter = 11
					end
			end //end else puf_enable

	end //end always #1

always @(negedge final_puf_osc[0] or negedge puf_enable)
	begin

		if ( !puf_enable )
			begin
				count0_to_20  <= 5'h0;
				drop0_counter <= 2'h0;
				block_clocks[0] <= 1'b0;
				puf_osc0_enable <= 5'h0;
			end
		else
			begin
				if ( count0_to_20 == 5'h13 )
					begin
						count0_to_20 <= 5'h0;
						drop0_counter <= drop0_counter + 1;
						block_clocks[0] <= 1'b0;
					end
				else
					begin
					   puf_osc0_enable <= {puf_osc0_enable[4:0],1'b1};
					   if ( puf_osc_reset )
						count0_to_20 <= count0_to_20 + 1;
					   if ( drop0_counter == 2'b00 )
						begin
							if ( count0_to_20 == drop_clocks[puf0_divider][4:0] )
								block_clocks[0] <= 1'b1;
						end //end of if drop_counter = 00
					   if ( drop0_counter == 2'b01 )
						begin
							if ( count0_to_20 == drop_clocks[puf0_divider][9:5] )
								block_clocks[0] <= 1'b1;
						end //end of if drop_counter = 01
					   if ( drop0_counter == 2'b10 )
						begin
							if ( count0_to_20 == drop_clocks[puf0_divider][14:10] )
								block_clocks[0] <= 1'b1;
						end //end of if drop_counter = 10
					   if ( drop0_counter == 2'b11 )
						begin
							if ( count0_to_20 == drop_clocks[puf0_divider][19:15] )
								block_clocks[0] <= 1'b1;
						end //end of if drop_counter = 11
					end
			end //end else puf_enable

	end //end always #0	
	
`endif //endif SLOW_SYSTEM_PUF_OSC_TEST - 10_5_25
//End JLPL - 10_1_25

// RPUF -------------------------------------------------------------------------------------------------------------------------

A2_RPUF_CoP #( //JL 
   .BASE  (ROPUF_base), //JLPL - 7_15_25
   .RANGE (ROPUF_range) //JLPL - 7_15-25
   ) i_RPUF (
   // A2S I/O interface
   .imosi      (imosi   ),
   .imiso      (imiso_rpuf), //JLPL - 9_15_25
   .intpt_puf  (intpt_rpuf), //JLPL - 9_7_25
   // Encryption Block Interface
   .pmosi      (kmosi),
   // Oscillator Control     //JLPL - 9_30_25
   .puf_enable (puf_enable ), //JLPL - 9_30_25
   .puf0_vector (puf0_vector), //JLPL - 9_30_25
   .puf1_vector (puf1_vector), //JLPL - 9_30_25
   .puf2_vector (puf2_vector), //JLPL - 9_30_25
   .puf3_vector (puf3_vector), //JLPL - 9_30_25
   .puf_osc_out (osc_out    ), //JLPL - 9_30_25
   // Oscillator Input
//   .puf_osc    (puf_osc), //JLPL - 10_1_25
`ifdef TEST_PUF_OSCILLATORS //JLPL - 10_1_25
   .puf_osc    (test_osc_out), //JLPL - 10_1_25   
`else                     //JLPL - 10_1_25
   .puf_osc    (final_puf_osc), //JLPL - 10_23_25
`endif                    //JLPL - 10_1_25
   // Global
   .reset      (zero_reset),
   .clock      (zero_clock)
   );
   

// GCM Authentication -----------------------------------------------------------------------------------------------------------

A2_GCM_CoP #( //JL
   .BASE     (GCM_base), //JL
   .NUM_REGS (GCM_range) //JL
   ) i_GCM ( //JL
   // Encryption Block Interface
   .ymosi      (ymosi_gf),
   .ymosi_fb   (ymosi_gf_fb), //***
   .ymiso      (ymiso),
   .ymiso_fb   (ymiso_gf_ir),    //JLSW //JLPL - 9_15_25
   .parity_error (gcm_parity_error), //JLPL - 11_8_25
   // A2S I/O interface
//   .imosi      (imosi   ), //JLPL - 10_21_25
	`ifdef RUN_PROCESSOR_TEST //JLPL - 10_9_25 - Go to ncverilog command line to define this test
		.imosi   (imosi_proc),                    // I/O Register Master-out slave-in //JLPL - 9_14_25
	`else
		.imosi   (imosi),                    // I/O Register Master-out slave-in //JLPL - 9_14_25
	`endif //endif RUN_PROCESSOR_TEST //JLPL - 10_9_25
   .imiso      (imiso_gcm), //JLPL - 9_15_25
//   .cc         (intpt_gcm), //JLPL - 9_7_25 //JLPL - 10_21_25
   .cc         (condc_gcm), //JLPL - 9_7_25 //JLPL - 10_23_25
   // Global
   .reset      (zero_reset),
//   .clock      (zero_clock) //JLPL - 10_21_25
	`ifdef RUN_PROCESSOR_TEST //JLPL - 10_9_25 - Go to ncverilog command line to define this test
		.clock      (zero_clock_boot) //JLPL - 9_5_25 //JLPL - 9_14_25
	`else
		.clock      (zero_clock) //JLPL - 9_14_25
	`endif //endif RUN_PROCESSOR_TEST //JLPL - 10_9_25
   );
   

// AES Encryption ---------------------------------------------------------------------------------------------------------------

wire [1:0] ymiso_rd = {ymiso_gf_ir,ymiso_fb_news}; //JLSW //JLPL - 9_15_25

A2_AES_CoP #( //JL
   .BASE     (AES_base), //JL
   .NUM_REGS (AES_range) //JL
   ) i_AES ( //JL
   .rmosi      (kmosi),                // Root if trust Key inputs from PUF
   // Encryption Block Interface
//   .nmosi      (ymosi_nn),             // Requests from NEWS //JLPL - 10_17_25
   .nmosi      ({ymosi_nn[4],1'b1,ymosi_nn[3:0]}),             // Requests from NEWS //JLPL - 10_17_25 - Change to new 6 bit input format - No change on NEWS side //JLPL - 10_18_25 set bit 4 = 1
   .nmosi_fb   (ymosi_nn_fb),          // Requests from NEWS take
   .gmosi      (ymosi_gf),             // Requests from GCM
   .gmosi_fb   (ymosi_gf_fb),          // Requests from GCM  take
   // Output to GCM and NEWS
   .amiso      (ymiso),
   .amiso_rd   (ymiso_rd),             // Takes
   // A2S I/O interface
   .imosi      (imosi   ),
   .imiso      (imiso_aes), //JLPL - 9_15_25
//   .intpt      (intpt_ae), //JL
   .intpt      (intpt_aes),   //JLPL - 9_7_25 //JLPL - 9_14_25 //JLPL - 9_18_25 - Restore to flag for older version //JLPL - 10_17_25 - Go back to intpt
   // Global
   .reset      (zero_reset),
   .clock      (zero_clock)
   );

// NEWS -------------------------------------------------------------------------------------------------------------------------

wire dummy_output; //JLSW
assign intp_news = intpt_news;  //JLPL
wire [8:0] nmosi_data_test; //JLPL
wire [ 4:0]   nm_lwr; //JLPL test workaround
wire [ 3:0]   nm_uppr; //JLPL test workaround
assign nmosi_data = {nm_uppr,nm_lwr}; //JLPL workaround

//JLPL - 8_27_25 - Test TileZero DMA RAM
wire [65:0] hmiso; //JLPL - 8_27_25
wire [33:0] bmiso_tst;
wire [66:0] hmosi = 67'h0;
wire [66:0] bmosi_tst = 67'h0;

A2_NEWS_CoP #(
   .BASE (NEWS_base) //JL - No NEWS_range for now
   ) i_NEWS ( //JL
   // A2S I/O interface
   //.imosi      (imosi_proc), //JLPL 9_5_25
`ifdef SIMULATE_CLOCK_RESET //JLPL - 9_8_25
	`ifdef RUN_PROCESSOR_TEST //JLPL - 10_9_25 - Go to ncverilog command line to define this test
		.imosi   (imosi_proc),                    // I/O Register Master-out slave-in //JLPL - 9_14_25
	`else
		.imosi   (imosi),                    // I/O Register Master-out slave-in //JLPL - 9_14_25
	`endif //endif RUN_PROCESSOR_TEST //JLPL - 10_9_25
`else
   .imosi   (imosi),                         // I/O Register Master-out slave-in
`endif //JLPL
   .imiso      (imiso_news), //JLPL - 9_15_25
   .intpt      (intpt_news), //JLPL - 9_7_25
   .intpt_ldma (intpt_ldma ), //JLPL - 9_6_25
   .nm_lwr     (nm_lwr), //JLPL breakout for compiler problem - Keep - 9_15_25
   .nm_uppr    (nm_uppr), //JLPL breakout for compiler problem - Keep - 9_15_25
   // NEWS output pipes
   .nmosi      (nmosi_data_test), .emosi      (emosi_data), .wmosi      (wmosi_data), .smosi      (smosi_data),       // NEWS Data //JL per Roger's interface name change in Cognitum_core
   .nmosi_give (nmosi_give), .emosi_give (emosi_give), .wmosi_give (wmosi_give), .smosi_give (smosi_give),  // NEWS give
   .nmosi_take (nmosi_take), .emosi_take (emosi_take), .wmosi_take (wmosi_take), .smosi_take (smosi_take),  // NEWS take
   .nmosi_clock(nmosi_clock),.emosi_clock(emosi_clock),.wmosi_clock(wmosi_clock),.smosi_clock(smosi_clock), // NEWS Clock
   .nmosi_reset(nmosi_reset),.emosi_reset(emosi_reset),.wmosi_reset(wmosi_reset),.smosi_reset(smosi_reset), // NEWS Reset
   // NEWS input  pipes
   .nmiso      (nmiso_data), .emiso      (emiso_data), .wmiso      (wmiso_data), .smiso      (smiso_data),       // NEWS Data //JL per Roger's interface name change in Cognitum_core
   .nmiso_give (nmiso_give), .emiso_give (emiso_give), .wmiso_give (wmiso_give), .smiso_give (smiso_give),  // NEWS give
   .nmiso_take (nmiso_take), .emiso_take (emiso_take), .wmiso_take (wmiso_take), .smiso_take (smiso_take),  // NEWS take
   .nmiso_clock (nmiso_clock), .emiso_clock (emiso_clock), .wmiso_clock (wmiso_clock), .smiso_clock (smiso_clock),  // NEWS take //JLSW 
   .nmiso_reset (nmiso_reset), .emiso_reset (emiso_reset), .wmiso_reset (wmiso_reset), .smiso_reset (smiso_reset),  // NEWS take //JLSW    
   // Recrypt Interface
   .rmosi      (ymosi_nn),
   .rmosi_fb   (ymosi_nn_fb),
   .rmiso      (ymiso),
   .rmiso_fb   (ymiso_fb_news), //JLSW
   .parity_error (news_parity_error), //JLPL - 11_8_25
   // Global
   .lmosi      (lmosi        ),                                             // 67 = {lreq, lwr, lrd, lwrd[31:0], ladr[31:0]} //JLSW -> To RAM_3PX DMA Port //JLPL - 8_27_25
   .lmiso      (lmiso        ),                                             //      {lack, lgnt, lrdd[31:0]}  = 34 //JLSW       -> From RAM_3PX DMA Port
   .dummy_output (dummy_output), //JLSW
   .break_input (tile_break), //JL
   .reset      (zero_reset),
	`ifdef RUN_PROCESSOR_TEST //JLPL - 10_9_25 - Go to ncverilog command line to define this test
		.clock      (zero_clock_boot) //JLPL - 9_5_25 //JLPL - 9_14_25
	`else
		.clock      (zero_clock) //JLPL - 9_14_25
	`endif //endif RUN_PROCESSOR_TEST //JLPL - 10_9_25
   );
   
   
//JTAG Interface Module //***

wire [47:0]   imosi_jtg; //***
//wire [32:0]   imiso_jtg; //*** //JLPL - 9_14_25

assign imosi_jtg =  trst ? 48'h555555555555 : 48'h055555555555; //*** Compile Test Value and preserve trst signal

// PVT --------------------------------------------------------------------------------------------------------------------------

A2_PVT_CoP #( //JL 
   .BASE     (PVT_base), //JL
   .NUM_REGS (PVT_range) //JL
   ) iPVT ( //JL
   .imiso(imiso_pvt),    // Slave data out {               IOk, IOo[31:0]} //JLPL - 9_15_25
//   .imosi(imosi_proc),       // Slave data in  {cIO[2:0] aIO[12:0], iIO[31:0]} //JLPL - 9_14_25 //JLPL - 10_22_25
`ifdef SIMULATE_CLOCK_RESET //JLPL
   .imosi   (imosi_proc),                    // I/O Register Master-out slave-in
`else
   .imosi   (imosi),                         // I/O Register Master-out slave-in
`endif //JLPL
   .pmosi(pmosi),       // PVT inputs     {CLK,TRIMG[4:0],TRIMO[5:0],ENA,VSAMPLE,PSAMPLE[1:0],}
   .pmiso(pmiso),       // PVT outputs    {DATA_VALID,DATA_OUT[9:0]}
   .intpt(intpt_pvt),   // Evaluation complete interrupt -- cleared by taking ENA low
   .reset(zero_reset),
   .clock(zero_clock_boot) //JLPL - 9_14_25
   ); 

A2_jtag_interface #(
   .BASE_ADDR(JTAG_base),       //JLPL - 9_14_25 //JLPL - 10_10_25
   //.NUM_REGS(JTAG_range),  //JLPL - 9_14_25 //JLPL - 10_10_25
   .JTAGID(32'h600df1d0)
   ) jtag_interface(  //JL
   // JTAG Tap Controller Interface -----------------------
   .tck   (tck),           // JTAG Test Clock
   .tms   (tms),           // JTAG Test Mode Select
   .tdi   (tdi),           // JTAG Test Data In
   .tdo   (tdo),           // JTAG Test Data Out
   // I/O Interface ----------------------------------------
   //.imosi (imosi_proc), //JLPL - 9_14_25 //JLPL - 10_22_25
	`ifdef RUN_PROCESSOR_TEST //JLPL - 10_9_25 - Go to ncverilog command line to define this test
		.imosi   (imosi_proc),                    // I/O Register Master-out slave-in //JLPL - 9_14_25
	`else
		.imosi   (imosi),                    // I/O Register Master-out slave-in //JLPL - 9_14_25
	`endif //endif RUN_PROCESSOR_TEST //JLPL - 10_9_25
   .imiso (imiso_jtag), //JLPL - 9_15_25
   .cond ({intpt_jtgi,intpt_jtgo}), //JLPL - 11_4_25
   // System Interface ------------------------------------
   //.clock (zero_clock_boot),         // System Clock //JLPL - 9_14_25 //JLPL - 10_22_25
	`ifdef RUN_PROCESSOR_TEST //JLPL - 10_9_25 - Go to ncverilog command line to define this test
		.clock      (zero_clock_boot), //JLPL - 9_5_25 //JLPL - 9_14_25
	`else
		.clock      (zero_clock), //JLPL - 9_14_25
	`endif //endif RUN_PROCESSOR_TEST //JLPL - 10_9_25
   .reset (zero_reset)          // System Reset
   );

//JLPL - 9_7_25   
A2_timer #(
   .BASE    (TMR0_base),
   .NUM_REGS(TMR0_range)
   ) i_TMR0 (
   .imiso   (imiso_tmr0[32:0]),     // Slave data out {               IOk, IOo[31:0]}
//   .imosi   (imosi_proc[47:0] ),         // Slave data in  {cIO[2:0] aIO[12:0], iIO[31:0]} //JLPL - 9_14_25 //JLPL - 10_22_25
`ifdef SIMULATE_CLOCK_RESET //JLPL
   .imosi   (imosi_proc),                    // I/O Register Master-out slave-in
`else
   .imosi   (imosi),                         // I/O Register Master-out slave-in
`endif //JLPL
   .intpt   (intpt_tmr0),          // Evaluation complete interrupt -- cleared by taking ENA low
   .clock   (zero_clock_boot), //JLPL - 9_7_25
   .reset   (zero_reset)
   );

A2_timer #(
   .BASE    (TMR1_base),
   .NUM_REGS(TMR1_range)
   ) i_TMR1 (
   .imiso   (imiso_tmr1[32:0]),     // Slave data out {               IOk, IOo[31:0]}
//   .imosi   (imosi_proc[47:0] ),         // Slave data in  {cIO[2:0] aIO[12:0], iIO[31:0]} //JLPL - 9_14_25 //JLPL - 10_22_25
`ifdef SIMULATE_CLOCK_RESET //JLPL
   .imosi   (imosi_proc),                    // I/O Register Master-out slave-in
`else
   .imosi   (imosi),                         // I/O Register Master-out slave-in
`endif //JLPL
   .intpt   (intpt_tmr1),           // Evaluation complete interrupt -- cleared by taking ENA low
   .clock   (zero_clock_boot), //JLPL - 9_7_25
   .reset   (zero_reset)
   );
  
assign flag_out[0] = 1'b0; //JLPL - 9_7_25  
flags #(
   .BASE    (FLAG_base),
   .NUM_REGS(2)
   ) i_FLAG (
   .imiso   (imiso_flag[32:0]),
//   .imosi   (imosi_proc[47:0]), //JLPL - 9_14_25 //JLPL - 10_22_25
`ifdef SIMULATE_CLOCK_RESET //JLPL
   .imosi   (imosi_proc),                    // I/O Register Master-out slave-in
`else
   .imosi   (imosi),                         // I/O Register Master-out slave-in
`endif //JLPL
   .flag_out(flag_out[255:1]), //JLPL - 9_7_25
   .flag_in (flag_in[255:1]), //JLPL - 9_7_25
   .intpt   (intpt_flag),
   .clock   (zero_clock_boot), //JLPL - 9_7_25
   .reset   (zero_reset)
   );
//End JLPL - 9_7_25

//JLPL - 11_12_25
timos #(
   .BASE    (TIMO_base),
   .NUM_REGS(1)
   ) i_TIMO (
   .imiso   (imiso_timo[32:0]),
   //.imosi   (imosi),                         // I/O Register Master-out slave-in //JLPL - 11_12_25
`ifdef SIMULATE_CLOCK_RESET //JLPL
   .imosi   (imosi_proc),                    // I/O Register Master-out slave-in
`else
   .imosi   (imosi),                         // I/O Register Master-out slave-in
`endif //JLPL
   .timo_in ({timo_in[255:1], timo_zero}),         //JLPL - 9_7_25
   .intpt   (intpt_timo),
   .clock   (zero_clock_boot),                     //JLPL - 9_7_25
   .reset   (zero_reset)
   );
//End JLPL - 11_12_25

// Tile Output -----------------------------------------------------------------------------------------------------------------
// XM responses have priority over GW requests


// Collate xmiso
//assign   xmiso    =  {1'b0, xordy, XMISO}; //***+++
//assign   xmiso[95:0]    =  XMISO; //JLPL

`ifdef SIMULATE_CLOCK_RESET //JLPL
//assign   xmiso[95:0] = tz_data[95:0]; //JLPL - 9_10_25
assign   xmiso[95:0] = xmiso_rz[95:0]; //JLPL - 9_10_25
//assign   xmiso_clk   = tz_clk; //JLPL - 9_10_25
assign   xmiso_clk   = xmiso_clk_rz; //JLPL - 9_10_25
`else
assign   xmiso[95:0] =  xmiso_rz[95:0]; //JLPL - 9_16_25
assign   xmiso_clk   = xmiso_clk_rz; //JLPL - 9_16_25
`endif

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

`ifdef  verify_tile_zero

module   t;

parameter   VROMSIZE =  65536,                  // Bytes
parameter   CODESIZE =  65536,                  // Bytes
parameter   DATASIZE =  65536,                  // Bytes
parameter   WORKSIZE =  16384,                  // Bytes
parameter   RESET_DELAY = 130                   // Time Units (picoseconds)

reg          nmiso_clock,emiso_clock,wmiso_clock,smiso_clock;  // NEWS Input  Pipeline Clock
reg          nmiso_give, emiso_give, wmiso_give, smiso_give;   // NEWS Input  Pipeline give
reg          nmiso_reset,emiso_reset,wmiso_reset,smiso_reset;  // NEWS Input  Pipeline Reset
reg          nmosi_take, emosi_take, wmosi_take, smosi_take;   // NEWS Output Pipeline take
reg          PoR;                                              // Power-On Reset
reg          PoR_bypass;                                       // Power-On Reset bypass
reg          PoR_extern;                                       // Power-On Reset external when bypassed
reg          tck;                                              //
reg          tdi;                                              //
reg          tms;                                              //
reg          xmiso_fb;                                         //          xpop
reg          xmosi_ck;                                         //
reg [  3:0]  puf_osc;                                          // Selected 4 of distributed oscillators for PUF
reg [  8:0]  nmiso,      emiso,      wmiso,      smiso;        // NEWS Input  Pipeline Inputs
reg [  8:0]  pmiso;                                            // PVT outputs  8 = { data_valid, data[7:0] }
reg [255:0]  flag_in;                                          // From Tile[N] //JL
reg [ 97:0]  xmosi;                                            // xresetn, xpush,

wire         nmiso_take, emiso_take, wmiso_take, smiso_take;   // NEWS Input  Pipeline take
wire         nmosi_clock,emosi_clock,wmosi_clock,smosi_clock;  // NEWS Output Pipeline Clock
wire         nmosi_give, emosi_give, wmosi_give, smosi_give;   // NEWS Output Pipeline give
wire         nmosi_reset,emosi_reset,wmosi_reset,smosi_reset;  // NEWS Output Pipeline Reset
wire         race_clock;                                       // Clock to North Hub of Raceway
wire         race_reset;                                       // Reset to North Hub of Raceway
wire         tdo;                                              // JTAG
wire         xmiso_ck;                                         //
wire         xmosi_fb;                                         //          xirdy
wire [  3:0] lvds_clock;                                       // Individual clocks for serial interfaces -- direct to DFEs (clocki)
wire [  8:0] nmosi,      emosi,      wmosi,      smosi;        // NEWS Output Pipeline Outputs
wire [ 15:0] pmosi;                                            // PVT inputs  16 = {clock,enable,psample[1:0],vsample,trimg[4:0],trimo[5:0]}
wire [255:0] flag_out;                                         // To   Tile[N] //JL
wire [ 97:0] xmiso;                                            // xtmo,    xordy, xack[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xrd0[31:0],xrd1[31:0]}

TileZero #(
   .VROMSIZE   (VROMSIZE   ),
   .CODESIZE   (CODESIZE   ),
   .DATASIZE   (DATASIZE   ),
   .WORKSIZE   (WORKSIZE   ),
   .RESET_DELAY(RESET_DELAY)
   )(
   // Output System Interconnect
   .xmiso_ck   (xmiso_ck),
   .xmiso      (xmiso),
   .xmiso_fb   (xmiso_fb),
   .xmosi_ck   (xmosi_ck),
   .xmosi      (xmosi),
   .xmosi_fb   (xmosi_fb),
   .nmosi      (nmosi),      .emosi       (emosi),      .wmosi       (wmosi),      .smosi       (smosi),
   .nmosi_give (nmosi_give), .emosi_give  (emosi_give), .wmosi_give  (wmosi_give), .smosi_give  (smosi_give),
   .nmosi_take (nmosi_take), .emosi_take  (emosi_take), .wmosi_take  (wmosi_take), .smosi_take  (smosi_take),
   .nmosi_clock(nmosi_clock),.emosi_clock (emosi_clock),.wmosi_clock (wmosi_clock),.smosi_clock (smosi_clock),
   .nmosi_reset(nmosi_reset),.emosi_reset (emosi_reset),.wmosi_reset (wmosi_reset),.smosi_reset (smosi_reset),
   .nmiso      (nmiso),      .emiso       (emiso),      .wmiso       (wmiso),      .smiso       (smiso),
   .nmiso_give (nmiso_give), .emiso_give  (emiso_give), .wmiso_give  (wmiso_give), .smiso_give  (smiso_give),
   .nmiso_take (nmiso_take), .emiso_take  (emiso_take), .wmiso_take  (wmiso_take), .smiso_take  (smiso_take),
   .nmiso_clock(nmiso_clock),.emiso_clock (emiso_clock),.wmiso_clock (wmiso_clock),.smiso_clock (smiso_clock),
   .nmiso_reset(nmiso_reset),.emiso_reset (emiso_reset),.wmiso_reset (wmiso_reset),.smiso_reset (smiso_reset),
   .pmosi      (pmosi),
   .pmiso      (pmiso),
   .tdo        (tdo),
   .tdi        (tdi)
   .tck        (tck),
   .tms        (tms),
   .lvds_clock (lvds_clock),
   .race_clock (race_clock),
   .race_reset (race_reset),
   .flag_out   (flag_out),
   .flag_in    (flag_in),
   .puf_osc    (puf_osc),
   .PoR_bypass (PoR_bypass),
   .PoR_extern (PoR_extern),
   .PoR        (PoR)
   );

initial begin
   nmiso       =   9'h0;
   emiso       =   9'h0;
   wmiso       =   9'h0;
   smiso       =   9'h0;
   nmiso_clock =   1'b0;
   emiso_clock =   1'b0;
   wmiso_clock =   1'b0;
   smiso_clock =   1'b0;
   nmiso_give  =   1'b0;
   emiso_give  =   1'b0;
   wmiso_give  =   1'b0;
   smiso_give  =   1'b0;
   nmiso_reset =   1'b0;
   emiso_reset =   1'b0;
   wmiso_reset =   1'b0;
   smiso_reset =   1'b0;
   nmosi_take  =   1'b0;
   emosi_take  =   1'b0;
   wmosi_take  =   1'b0;
   smosi_take  =   1'b0;
   PoR         =   1'b0;
   PoR_bypass  =   1'b0;
   PoR_extern  =   1'b0;
   tck         =   1'b0;
   tdi         =   1'b0;
   tms         =   1'b0;
   xmiso_fb    =   1'b0;
   xmosi_ck    =   1'b0;
   puf_osc     =   4'h0;
   pmiso       =   9'h0;
   flag_in     = 255'h0;
   xmosi       =  98'h0;
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
