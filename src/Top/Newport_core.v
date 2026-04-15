//===============================================================================================================================
//
//    Copyright � 2021..2025 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to Authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//    Project Name         : Cognitum
//
//    Description          : core of 256 processor array
//
//===============================================================================================================================
//    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//
//    NOTES:
//
//===============================================================================================================================

module   Cognitum_core #(
   parameter   NORTH_PIPE_DEPTH =  1,
   parameter    EAST_PIPE_DEPTH =  1,
   parameter    WEST_PIPE_DEPTH =  1,
   parameter   SOUTH_PIPE_DEPTH =  1
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
   output wire gpio_0_di, //JLSW
   output wire gpio_4_di, //JLSW
   output wire gpio_8_di, //JLSW
   output wire gpio_12_di, //JLSW
   input wire gpio_0_do, //JLSW
   input wire gpio_4_do, //JLSW
   input wire gpio_8_do, //JLSW
   input wire gpio_12_do, //JLSW
   output wire gpio_0_oe, //JL
   output wire gpio_4_oe, //JL
   output wire gpio_8_oe, //JL
   output wire gpio_12_oe, //JL

   output wire gpio_1_di, //JLSW
   output wire gpio_5_di, //JLSW
   output wire gpio_9_di, //JLSW
   output wire gpio_13_di, //JLSW
   input wire gpio_1_do, //JLSW
   input wire gpio_5_do, //JLSW
   input wire gpio_9_do, //JLSW
   input wire gpio_13_do, //JLSW
   output wire gpio_1_oe, //JL
   output wire gpio_5_oe, //JL
   output wire gpio_9_oe, //JL
   output wire gpio_13_oe, //JL

   output wire gpio_2_di, //JLSW
   output wire gpio_6_di, //JLSW
   output wire gpio_10_di, //JLSW
   output wire gpio_14_di, //JLSW
   input wire gpio_2_do, //JLSW
   input wire gpio_6_do, //JLSW
   input wire gpio_10_do, //JLSW
   input wire gpio_14_do, //JLSW
   output wire gpio_2_oe, //JL
   output wire gpio_6_oe, //JL
   output wire gpio_10_oe, //JL
   output wire gpio_14_oe, //JL

   output wire gpio_3_di, //JLSW
   output wire gpio_7_di, //JLSW
   output wire gpio_11_di, //JLSW
   output wire gpio_15_di, //JLSW
   input wire gpio_3_do, //JLSW
   input wire gpio_7_do, //JLSW
   input wire gpio_11_do, //JLSW
   input wire gpio_15_do, //JLSW
   output wire gpio_3_oe, //JL
   output wire gpio_7_oe, //JL
   output wire gpio_11_oe, //JL
   output wire gpio_15_oe, //JL

// Single-ended Rx\Tx from DFE to AFE
   input wire        WRXD_se,             SRXD_se,             ERXD_se,             NRXD_se,    // SDI_se Diff Rx Data
   input wire        WRXC_se,             SRXC_se,             ERXC_se,             NRXC_se,    // SCI_se Diff Rx Clk
   input wire        WRXC_seb,            SRXC_seb,            ERXC_seb,            NRXC_seb,   // MAV (5Feb25) new //SCI_se_n Inv Diff Rx Clk
   output wire       WTXD_se,             STXD_se,             ETXD_se,             NTXD_se,    // SCO_se Diff Tx Clk
   output wire       WTXC_se,             STXC_se,             ETXC_se,             NTXC_se,    // SDO_se Diff Tx Data
// AFE  Control
   output wire [5:0] Wrx_rterm_trim,      Srx_rterm_trim,      Erx_rterm_trim,      Nrx_rterm_trim,
   output wire [3:0] Wrx_offset_trim,     Srx_offset_trim,     Erx_offset_trim,     Nrx_offset_trim,
   output wire [4:0] Wrx_cdac_p,          Srx_cdac_p,          Erx_cdac_p,          Nrx_cdac_p,
   output wire [4:0] Wrx_cdac_m,          Srx_cdac_m,          Erx_cdac_m,          Nrx_cdac_m,
   output wire [3:0] Wrx_bias_trim,       Srx_bias_trim,       Erx_bias_trim,       Nrx_bias_trim,
   output wire [3:0] Wrx_gain_trim,       Srx_gain_trim,       Erx_gain_trim,       Nrx_gain_trim,
   output wire [3:0] Wtx_bias_trim,       Stx_bias_trim,       Etx_bias_trim,       Ntx_bias_trim,
   output wire [3:0] Wtx_offset_trim,     Stx_offset_trim,     Etx_offset_trim,     Ntx_offset_trim,
   output wire [5:0] Wpmu_bias_trim,      Spmu_bias_trim,      Epmu_bias_trim,      Npmu_bias_trim,
   output wire [5:0] Wlvds_spare_in,      Slvds_spare_in,      Elvds_spare_in,      Nlvds_spare_in,
   input wire  [7:0] Wlvds_spare_out,     Slvds_spare_out,     Elvds_spare_out,     Nlvds_spare_out,
   output wire [4:0] Wanalog_test,        Sanalog_test,        Eanalog_test,        Nanalog_test,
   output wire       Wanalog_reserved,    Sanalog_reserved,    Eanalog_reserved,    Nanalog_reserved,
   output wire       Wpreserve_spare_net, Spreserve_spare_net, Epreserve_spare_net, Npreserve_spare_net, //JL
   output wire       Wpower_down,         Spower_down,         Epower_down,         Npower_down, //JL
// JTAG
   input wire        tck,
   output wire       tdo,
   input wire        tdi,
   input wire        tms,
   input wire        trst,
// POR
   input  wire       sync_phase, //JLPL - 10_23_25
   input  wire       PoR_bypass,
   input  wire       PoR_extern,
   input wire        POR       //JL                 
   );

// PARAMETERS ===================================================================================================================

localparam  WEST = 0, SOUTH = 1, EAST = 2, NORTH = 3;

// LOCALS =======================================================================================================================

genvar    i,j;


//***JL Comment Out Change - Compile Errors - Revert to Flat Defintion that will Compile
////I/O Mapping
//wire
//   Wgpio_di[3:0] = {gpio_3_di, gpio_2_di, gpio_1_di, gpio_0_di},
//   Wgpio_do[3:0] = {gpio_3_do, gpio_2_do, gpio_1_do, gpio_0_do},
//   Wgpio_oe[3:0] = {gpio_3_oe, gpio_2_oe, gpio_1_oe, gpio_0_oe},
//
//   Sgpio_di[3:0] = {gpio_7_di, gpio_6_di, gpio_5_di, gpio_4_di},
//   Sgpio_do[3:0] = {gpio_7_do, gpio_6_do, gpio_5_do, gpio_4_do},
//   Sgpio_oe[3:0] = {gpio_7_oe, gpio_6_oe, gpio_5_oe, gpio_4_oe},
//
//   Egpio_di[3:0] = {gpio_11_di,gpio_10_di,gpio_9_di, gpio_8_di},
//   Egpio_do[3:0] = {gpio_11_do,gpio_10_do,gpio_9_do, gpio_8_do},
//   Egpio_oe[3:0] = {gpio_11_oe,gpio_10_oe,gpio_9_oe, gpio_8_oe},
//
//   Ngpio_di[3:0] = {gpio_15_di,gpio_14_di,gpio_13_di,gpio_12_di},
//   Ngpio_do[3:0] = {gpio_15_do,gpio_14_do,gpio_13_do,gpio_12_do},
//   Ngpio_oe[3:0] = {gpio_15_oe,gpio_14_oe,gpio_13_oe,gpio_12_oe};
////End I/O Mapping
//***JL End of Comment Out

//***JL - Revert to Flat Defintion that compiles
//I/O Mapping
wire [3:0] Wgpio_di;
wire [3:0] Sgpio_di;
wire [3:0] Egpio_di;
wire [3:0] Ngpio_di;
wire [3:0] Wgpio_do;
wire [3:0] Sgpio_do;
wire [3:0] Egpio_do;
wire [3:0] Ngpio_do;
wire [3:0] Wgpio_oe;
wire [3:0] Sgpio_oe;
wire [3:0] Egpio_oe;
wire [3:0] Ngpio_oe;

assign gpio_0_di = Wgpio_di[0]; //JLSW
assign gpio_1_di = Wgpio_di[1]; //JLSW
assign gpio_2_di = Wgpio_di[2]; //JLSW
assign gpio_3_di = Wgpio_di[3]; //JLSW
assign Wgpio_do[0] = gpio_0_do;
assign Wgpio_do[1] = gpio_1_do;
assign Wgpio_do[2] = gpio_2_do;
assign Wgpio_do[3] = gpio_3_do;
assign gpio_0_oe = Wgpio_oe[0]; //JLSW
assign gpio_1_oe = Wgpio_oe[1]; //JLSW
assign gpio_2_oe = Wgpio_oe[2]; //JLSW
assign gpio_3_oe = Wgpio_oe[3]; //JLSW

assign gpio_4_di = Sgpio_di[0]; //JLSW
assign gpio_5_di = Sgpio_di[1]; //JLSW
assign gpio_6_di = Sgpio_di[2]; //JLSW
assign gpio_7_di = Sgpio_di[3]; //JLSW
assign Sgpio_do[0] = gpio_4_do;
assign Sgpio_do[1] = gpio_5_do;
assign Sgpio_do[2] = gpio_6_do;
assign Sgpio_do[3] = gpio_7_do;
assign gpio_4_oe = Sgpio_oe[0]; //JLSW
assign gpio_5_oe = Sgpio_oe[1]; //JLSW
assign gpio_6_oe = Sgpio_oe[2]; //JLSW
assign gpio_7_oe = Sgpio_oe[3]; //JLSW

assign gpio_8_di = Egpio_di[0]; //JLSW
assign gpio_9_di = Egpio_di[1]; //JLSW
assign gpio_10_di = Egpio_di[2]; //JLSW
assign gpio_11_di = Egpio_di[3]; //JLSW
assign Egpio_do[0] = gpio_8_do;
assign Egpio_do[1] = gpio_9_do;
assign Egpio_do[2] = gpio_10_do;
assign Egpio_do[3] = gpio_11_do;
assign gpio_8_oe = Egpio_oe[0]; //JLSW
assign gpio_9_oe = Egpio_oe[1]; //JLSW
assign gpio_10_oe = Egpio_oe[2]; //JLSW
assign gpio_11_oe = Egpio_oe[3]; //JLSW

assign gpio_12_di = Ngpio_di[0]; //JLSW
assign gpio_13_di = Ngpio_di[1]; //JLSW
assign gpio_14_di = Ngpio_di[2]; //JLSW
assign gpio_15_di = Ngpio_di[3]; //JLSW
assign Ngpio_do[0] = gpio_12_do;
assign Ngpio_do[1] = gpio_13_do;
assign Ngpio_do[2] = gpio_14_do;
assign Ngpio_do[3] = gpio_15_do;
assign gpio_12_oe = Ngpio_oe[0]; //JLSW
assign gpio_13_oe = Ngpio_oe[1]; //JLSW
assign gpio_14_oe = Ngpio_oe[2]; //JLSW
assign gpio_15_oe = Ngpio_oe[3]; //JLSW

//End I/O Mapping
//***JL End of Flat Definition that will compile


// NEWS : DFE  Transmit Pipeline
wire  [  8:0]  N_txdi, E_txdi, W_txdi, S_txdi;  // from NEWS
wire  [  8:0]  N_txdo, E_txdo, W_txdo, S_txdo;  //  to  DFE
wire           N_txwr, E_txwr, W_txwr, S_txwr;
wire           N_txir, E_txir, W_txir, S_txir;
wire           N_txrd, E_txrd, W_txrd, S_txrd;
wire           N_txor, E_txor, W_txor, S_txor;

// DFE : NEWS  Receive Pipeline
wire  [  8:0]  N_rxdi, E_rxdi, W_rxdi, S_rxdi;  // from DFE
wire  [  8:0]  N_rxdo, E_rxdo, W_rxdo, S_rxdo;  //  to  NEWS
wire           N_rxwr, E_rxwr, W_rxwr, S_rxwr;
wire           N_rxir, E_rxir, W_rxir, S_rxir;
wire           N_rxrd, E_rxrd, W_rxrd, S_rxrd;
wire           N_rxor, E_rxor, W_rxor, S_rxor;

// Quadrant I/O
wire [  3:0]  QPUFosc [0:3]; //JL
wire [ 63:0]  QTIMOo [0:3]; //JL
wire  [383:0]  QI    [0:3];
wire  [  3:0]  QI_wr [0:3]; //JL
wire  [  3:0]  QI_ir [0:3]; //JL
wire  [  0:3]  QI_ck ;

wire  [383:0]  QO    [0:3];
wire  [  3:0]  QO_or [0:3]; //JL
wire  [  3:0]  QO_rd [0:3]; //JL
wire  [  0:3]  QO_ck ;

// Hub I/O
wire  [383:0]  HI;
wire  [  3:0]  HI_or;
wire  [  3:0]  HI_rd;
wire           HI_ck;
wire           HI_reset; //JL
wire  [383:0]  HO;
wire  [  3:0]  HO_wr;
wire  [  3:0]  HO_ir;
wire           HO_ck;
wire           HO_reset; //JL

// PVT
wire  [10:0]   pmiso;
wire  [15:0]   pmosi;

// DFE Spare - Keep Internal ( Via loopback ) to Cognitum.core netlist for Now
wire [3:0]     dfe_west_spare_output, dfe_south_spare_output, dfe_east_spare_output, dfe_north_spare_output;
//wire [3:0]   dfe_west_spare_input,  dfe_south_spare_input,  dfe_east_spare_input,  dfe_north_spare_input;

// Quadrant and TileZero signals defined early before used below - JLSW
wire TR_zero_ck; //JLSWT
wire TR_zero_fb; //JLSW
wire RT_zero_ck; //JLSW
wire RT_zero_fb; //JLSW
wire [97:0] TR_zero; //JLSW
wire [97:0] RT_zero; //JLSW

// Hub signals defined early before used below - JLSW
wire [3:0] HO_or;
wire [3:0] HO_rd;
wire [3:0] HI_wr;
wire [3:0] HI_ir;

wire W_txck,S_txck,E_txck,N_txck; //JLSW
wire W_txck_pipe_out,S_txck_pipe_out,E_txck_pipe_out,N_txck_pipe_out; //JLSW
wire W_txrs,S_txrs,E_txrs,N_txrs; //JLSW
wire W_txrs_pipe_out,S_txrs_pipe_out,E_txrs_pipe_out,N_txrs_pipe_out; //JLSW

wire W_clki,S_clki,E_clki,N_clki; //JLSW
wire wmiso_clock,smiso_clock,emiso_clock,nmiso_clock; //JLSW
wire wmiso_reset,smiso_reset,emiso_reset,nmiso_reset; //JLSW

// INSTANCES ====================================================================================================================

// DFE WEST
   lvds_dfe iWdfe (
      // CLOCK/RESET
`ifdef SIMULATE_CLOCK_RESET //JLSW
      .clocki         ( clocki ),                  // Internal  Serial output clock
`else
      .clocki         ( W_clki ),                  // Internal  Serial output clock
`endif //JLSW - endif SIMULATE_CLOCK_RESET
      .por            ( POR      ),                      // Power-On  reset
      // INPUTS
      .SCI_se         ( WRXC_se  ),
      .SCI_se_n       ( WRXC_seb ),
      .SDI_se         ( WRXD_se  ),
      // OUTPUTS
      .SCO_se         ( WTXC_se  ),
      .SDO_se         ( WTXD_se  ),
      // NEWS Transmit
      .input_ready    ( W_txrd   ), // Internal output
      .TxDataIn       ( W_txdo   ), // Internal input
      .write          ( W_txor   ), // Internal input
      .clockd         ( W_txck_pipe_out   ), // Internal input //JLSW
      .reset          ( W_txrs_pipe_out   ), // Internal input from System reset //JLSW
      // NEWS Receive
      .read           ( W_rxir   ), // Internal input
      .DataOut        ( W_rxdi   ), // Internal output
      .output_ready   ( W_rxwr   ), // Internal output
      .clocko         ( W_rxck   ), // Internal output
      // CONFIG
      .power_down     ( Wpower_down       ),
      .rx_rterm_trim  ( Wrx_rterm_trim    ),
      .rx_offset_trim ( Wrx_offset_trim   ),
      .rx_cdac_p      ( Wrx_cdac_p        ),
      .rx_cdac_m      ( Wrx_cdac_m        ),
      .rx_bias_trim   ( Wrx_bias_trim     ),
      .rx_gain_trim   ( Wrx_gain_trim     ),
      .tx_bias_trim   ( Wtx_bias_trim     ),
      .tx_offset_trim ( Wtx_offset_trim   ),
      .pmu_bias_trim  ( Wpmu_bias_trim    ),
      .analog_test    ( Wanalog_test      ),
      .analog_reserved( Wanalog_reserved  ),
      // SPARE
      .lvds_spare_in  ( Wlvds_spare_in    ),
      .lvds_spare_out ( Wlvds_spare_out   ),
      .preserve_spare_net (/*unconncted*/),
      // GPIO
      .gpio_rxData    ( Wgpio_do ), //JLSW
      .gpio_txData    ( Wgpio_di ), //JLSW
      .gpio_outEn     ( Wgpio_oe ), //JL
     .spare_input    (dfe_west_spare_output), //JL
     .spare_output   (dfe_west_spare_output)  //JL
      );  // iWdfe

//===============================================================================================================================
// DFE SOUTH
   lvds_dfe iSdfe (
      // CLOCK/RESET
`ifdef SIMULATE_CLOCK_RESET //JLSW
      .clocki         ( clocki ),                  // Internal  Serial output clock
`else
      .clocki         ( S_clki ),                  // Internal  Serial output clock
`endif //JLSW - endif SIMULATE_CLOCK_RESET
      .por            ( POR ),            // Power-On  reset
      // INPUTS
      .SCI_se         ( SRXC_se  ),
      .SCI_se_n       ( SRXC_seb ),
      .SDI_se         ( SRXD_se  ),
      // OUTPUTS
      .SCO_se         ( STXC_se  ),
      .SDO_se         ( STXD_se  ),
      // NEWS Transmit
      .input_ready    ( S_txrd   ),   // Internal output
      .TxDataIn       ( S_txdo   ),   // Internal input
      .write          ( S_txor   ),   // Internal input
      .clockd         ( S_txck_pipe_out   ),   // Internal input //JLSW
      .reset          ( S_txrs_pipe_out   ),   // Internal input from System reset //JLSW
      // NEWS Receive
      .read           ( S_rxir   ),   // Internal input
      .DataOut        ( S_rxdi   ),   // Internal output
      .output_ready   ( S_rxwr   ),   // Internal output
      .clocko         ( S_rxck   ),   // Internal output
      // CONFIG
      .power_down     ( Spower_down       ),
      .rx_rterm_trim  ( Srx_rterm_trim    ),
      .rx_offset_trim ( Srx_offset_trim   ),
      .rx_cdac_p      ( Srx_cdac_p        ),
      .rx_cdac_m      ( Srx_cdac_m        ),
      .rx_bias_trim   ( Srx_bias_trim     ),
      .rx_gain_trim   ( Srx_gain_trim     ),
      .tx_bias_trim   ( Stx_bias_trim     ),
      .tx_offset_trim ( Stx_offset_trim   ),
      .pmu_bias_trim  ( Spmu_bias_trim    ),
      .analog_test    ( Sanalog_test      ),
      .analog_reserved( Sanalog_reserved  ),
      // SPARE
      .lvds_spare_in  ( Slvds_spare_in    ),
      .lvds_spare_out ( Slvds_spare_out   ),
      .preserve_spare_net (/*unconncted*/ ),
      // GPIO
      .gpio_rxData    ( Sgpio_do ), //JLSW
      .gpio_txData    ( Sgpio_di ), //JLSW
      .gpio_outEn     ( Sgpio_oe ), //JL
     .spare_input    (dfe_south_spare_output), //JL
     .spare_output   (dfe_south_spare_output)  //JL
      );  // iSdfe

//===============================================================================================================================
// DFE EAST
   lvds_dfe iEdfe (
      // CLOCK/RESET
`ifdef SIMULATE_CLOCK_RESET //JLSW
      .clocki         ( clocki ),                  // Internal  Serial output clock
`else
      .clocki         (E_clki),                    // Internal  Serial output clock
`endif //JLSW - endif SIMULATE_CLOCK_RESET
      .por            (POR),           // Power-On  reset
      // INPUTS
      .SCI_se         (ERXC_se),
      .SCI_se_n       (ERXC_seb),
      .SDI_se         (ERXD_se),
      // OUTPUTS
      .SCO_se         (ETXC_se),
      .SDO_se         (ETXD_se),
      // NEWS Transmit
      .input_ready    ( E_txrd   ), // Internal output
      .TxDataIn       ( E_txdo   ), // Internal input
      .write          ( E_txor   ), // Internal input
      .clockd         ( E_txck_pipe_out   ), // Internal input //JLSW
      .reset          ( E_txrs_pipe_out   ), // Internal input from System reset //JLSW
      // NEWS Receive
      .read           ( E_rxir   ), // Internal input
      .DataOut        ( E_rxdi   ), // Internal output
      .output_ready   ( E_rxwr   ), // Internal output
      .clocko         ( E_rxck   ), // Internal output
      // CONFIG
      .power_down     ( Epower_down),
      .rx_rterm_trim  ( Erx_rterm_trim),
      .rx_offset_trim ( Erx_offset_trim),
      .rx_cdac_p      ( Erx_cdac_p),
      .rx_cdac_m      ( Erx_cdac_m),
      .rx_bias_trim   ( Erx_bias_trim),
      .rx_gain_trim   ( Erx_gain_trim),
      .tx_bias_trim   ( Etx_bias_trim),
      .tx_offset_trim ( Etx_offset_trim),
      .pmu_bias_trim  ( Epmu_bias_trim),
      .analog_test    ( Eanalog_test),
      .analog_reserved( Eanalog_reserved),
      // SPARE
      .lvds_spare_in  ( Elvds_spare_in),
      .lvds_spare_out ( Elvds_spare_out),
      .preserve_spare_net (/*unconncted*/),
      // GPIO
      .gpio_rxData    ( Egpio_do ), //JLSW
      .gpio_txData    ( Egpio_di ), //JLSW
      .gpio_outEn     ( Egpio_oe ), //JL
      .spare_input    (dfe_east_spare_output), //JL
      .spare_output   (dfe_east_spare_output)  //JL
      );  // iEdfe

//===============================================================================================================================
// DFE NORTH
   lvds_dfe iNdfe (
      // CLOCK/RESET
`ifdef SIMULATE_CLOCK_RESET //JLSW
      .clocki         ( clocki ),                  // Internal  Serial output clock
`else
      .clocki         (N_clki),                    // Internal  Serial output clock
`endif //JLSW - endif SIMULATE_CLOCK_RESET
      .por            (POR),           // Power-On  reset
      // INPUTS
      .SCI_se         (NRXC_se),
      .SCI_se_n       (NRXC_seb),
      .SDI_se         (NRXD_se),
      // OUTPUTS
      .SCO_se         (NTXC_se),
      .SDO_se         (NTXD_se),
      // NEWS Transmit
      .input_ready    ( N_txrd   ), // Internal output
      .TxDataIn       ( N_txdo   ), // Internal input
      .write          ( N_txor   ), // Internal input
      .clockd         ( N_txck_pipe_out   ), // Internal input //JLSW
      .reset          ( N_txrs_pipe_out   ), // Internal input from System reset //JLSW
      // NEWS Receive
      .read           ( N_rxir   ), // Internal input
      .DataOut        ( N_rxdi   ), // Internal output
      .output_ready   ( N_rxwr   ), // Internal output
      .clocko         ( N_rxck   ), // Internal output
      // CONFIG
      .power_down     ( Npower_down),
      .rx_rterm_trim  ( Nrx_rterm_trim),
      .rx_offset_trim ( Nrx_offset_trim),
      .rx_cdac_p      ( Nrx_cdac_p),
      .rx_cdac_m      ( Nrx_cdac_m),
      .rx_bias_trim   ( Nrx_bias_trim),
      .rx_gain_trim   ( Nrx_gain_trim),
      .tx_bias_trim   ( Ntx_bias_trim),
      .tx_offset_trim ( Ntx_offset_trim),
      .pmu_bias_trim  ( Npmu_bias_trim),
      .analog_test    ( Nanalog_test),
      .analog_reserved( Nanalog_reserved),
      // SPARE
      .lvds_spare_in  ( Nlvds_spare_in),
      .lvds_spare_out ( Nlvds_spare_out),
      .preserve_spare_net (/*unconncted*/),
      // GPIO
      .gpio_rxData    ( Ngpio_do ), //JLSW
      .gpio_txData    ( Ngpio_di ), //JLSW
      .gpio_outEn     ( Ngpio_oe ), //JL
      .spare_input    (dfe_north_spare_output), //JL
      .spare_output   (dfe_north_spare_output)  //JL
      );  // iNdfe

//*** DFE : NEWS Pipeline Compile WorkAround

wire [1:0] greg_force;     //***JL
wire clock_out;          //***JL
wire reset_out;          //***JL

// Pipelines between DFEs and TileZero ==========================================================================================

wire [8:0] break_input; //JL
assign   break_input  = 9'h1f7;  // "Break"  K code : K23.7  0xF7.  bit-8 must be set! //JL

A2_pipeline #(9,NORTH_PIPE_DEPTH) iPNO (.pdi(N_txdi),.pwr(N_txwr),.pir(N_txir),                             // from NEWS
                                        .pdo(N_txdo),.por(N_txor),.prd(N_txrd),                             // to   DFE
                                        .pfm(break_input ),.cki(N_txck),.rsi(N_txrs),.cko(N_txck_pipe_out),.rso(N_txrs_pipe_out)); //JLSW
A2_pipeline #(9, EAST_PIPE_DEPTH) iPEO (.pdi(E_txdi),.pwr(E_txwr),.pir(E_txir),
                                        .pdo(E_txdo),.por(E_txor),.prd(E_txrd),
                                        .pfm(break_input ),.cki(E_txck),.rsi(E_txrs),.cko(E_txck_pipe_out),.rso(E_txrs_pipe_out)); //JLSW
A2_pipeline #(9, WEST_PIPE_DEPTH) iPWO (.pdi(W_txdi),.pwr(W_txwr),.pir(W_txir),
                                        .pdo(W_txdo),.por(W_txor),.prd(W_txrd),
                                        .pfm(break_input ),.cki(W_txck),.rsi(W_txrs),.cko(W_txck_pipe_out),.rso(W_txrs_pipe_out)); //JLSW
A2_pipeline #(9,SOUTH_PIPE_DEPTH) iPSO (.pdi(S_txdi),.pwr(S_txwr),.pir(S_txir),
                                        .pdo(S_txdo),.por(S_txor),.prd(S_txrd),
                                        .pfm(break_input ),.cki(S_txck),.rsi(S_txrs),.cko(S_txck_pipe_out),.rso(S_txrs_pipe_out)); //JLSW

A2_pipeline #(9,NORTH_PIPE_DEPTH) iPNI (.pdi(N_rxdi),.pwr(N_rxwr),.pir(N_rxir),                             // from DFE
                                        .pdo(N_rxdo),.por(N_rxor),.prd(N_rxrd),                             // to   NEWS
                                        .pfm(break_input ),.cki(N_rxck),.rsi(N_txrs),.cko(nmiso_clock),.rso(nmiso_reset)); //JLSW //JLPL - 11_11_25  Change to rxck
A2_pipeline #(9, EAST_PIPE_DEPTH) iPEI (.pdi(E_rxdi),.pwr(E_rxwr),.pir(E_rxir),
                                        .pdo(E_rxdo),.por(E_rxor),.prd(E_rxrd),
                                        .pfm(break_input ),.cki(E_rxck),.rsi(E_txrs),.cko(emiso_clock),.rso(emiso_reset)); //JLSW //JLPL - 11_11_25  Change to rxck
A2_pipeline #(9, WEST_PIPE_DEPTH) iPWI (.pdi(W_rxdi),.pwr(W_rxwr),.pir(W_rxir),
                                        .pdo(W_rxdo),.por(W_rxor),.prd(W_rxrd),
                                        .pfm(break_input ),.cki(W_rxck),.rsi(W_txrs),.cko(wmiso_clock),.rso(wmiso_reset)); //JLSW //JLPL - 11_11_25  Change to rxck
A2_pipeline #(9,SOUTH_PIPE_DEPTH) iPSI (.pdi(S_rxdi),.pwr(S_rxwr),.pir(S_rxir),
                                        .pdo(S_rxdo),.por(S_rxor),.prd(S_rxrd),
                                        .pfm(break_input ),.cki(S_rxck),.rsi(S_txrs),.cko(smiso_clock),.rso(smiso_reset)); //JLSW //JLPL - 11_11_25  Change to rxck

// QuadrantZero  NW,  QuadrantOnes NE, SW, & SE ================================================================================-

wire [ 63:0]  FLAGo [0:3]; //JL
wire [ 63:0]  FLAGi [0:3]; //JL
wire quadrant_reset; //JLSW - temporary

`ifdef SIMULATE_CLOCK_RESET //JLSW
    assign quadrant_reset = POR; //JLSW - temporary
`else
assign   quadrant_reset = POR; //JLSW - temporary
`endif //JLSW - endif SIMULATE_CLOCK_RESET

for (i=0; i<4; i=i+1) begin : C
   if (i==0) begin : CA
      QuadrantZero iQZ (
         // Hub interface
         .QO         (QO   [i]),
         .QO_or      (QO_or[i]),
         .QO_rd      (QO_rd[i]),
         .QO_ck      (QO_ck[i]),
         .QI         (QI   [i]),
         .QI_wr      (QI_wr[i]),
         .QI_ir      (QI_ir[i]),
         .QI_ck      (QI_ck[i]),
         // TileZero interface
         .RT_zero    (RT_zero),
         .RT_zero_fb (RT_zero_fb),
         .RT_zero_ck (RT_zero_ck),
         .TR_zero    (TR_zero), //JL
         .TR_zero_fb (TR_zero_fb),
         .TR_zero_ck (TR_zero_ck),
         // Global
		 .QPUFosc    (QPUFosc  [i]), //JL
		 .QTIMOo     (QTIMOo   [i]), //JL
         .FLAGo      (FLAGo[i]),     //JL
         .FLAGi      (FLAGi[i]),     //JL
         .reset      (quadrant_reset) //JLSW - temporary
         );
      end
   else begin : CB
`ifdef synthesis
      Quadrant iQ1 (   //JL - With Quadrant Breakdown need to evaluate bit ordering of Qid ?
`else
      Quadrant #(i) iQ1 (   //JL - With Quadrant Breakdown need to evaluate bit ordering of Qid ?
`endif
         // Hub interface
         .QO      (QO   [i]),
         .QO_or   (QO_or[i]),
         .QO_rd   (QO_rd[i]),
         .QO_ck   (QO_ck[i]),
         .QI      (QI   [i]),
         .QI_wr   (QI_wr[i]),
         .QI_ir   (QI_ir[i]),
         .QI_ck   (QI_ck[i]),
         // Global
		 .QPUFosc (QPUFosc [i]), //JL
		 .QTIMOo  (QTIMOo  [i]), //JL
         .QFLAGo   (FLAGo[i]), //JL
         .QFLAGi   (FLAGi[i]), //JL
         .reset   (quadrant_reset) //JLSW - temporary
         );
      end
   end

// HUB NORTH & HUB SOUTH ========================================================================================================

//JL - Reset Connections
wire QI_reset_Hub_N_W;
wire QO_reset_Hub_N_W;
wire QI_reset_Hub_N_E;
wire QO_reset_Hub_N_E;
wire QI_reset_Hub_S_W;
wire QO_reset_Hub_S_W;
wire QI_reset_Hub_S_E;
wire QO_reset_Hub_S_E;
//JL - End Reset Connections
wire race_reset; //JLPL - 9_14_25

`ifdef SIMULATE_CLOCK_RESET //JLSW
    assign QO_reset_Hub_N_W  = POR | race_reset; //JLPL - temporary; //JLPL - 9_14_25 - Need Reset from the Quadrants
    assign QO_reset_Hub_N_E  = POR | race_reset; //JLPL - temporary; //JLPL - 9_14_25 - Need Reset from the Quadrants
    assign QO_reset_Hub_S_W  = POR | race_reset; //JLPL - temporary; //JLPL - 9_14_25 - Need Reset from the Quadrants
    assign QO_reset_Hub_S_E  = POR | race_reset; //JLPL - temporary; //JLPL - 9_14_25 - Need Reset from the Quadrants
`else
assign   QO_reset_Hub_N_W  = POR | race_reset; //JLPL - temporary; //JLPL - 9_23_25 - Need Reset from the Quadrants 
assign   QO_reset_Hub_N_E  = POR | race_reset; //JLPL - temporary; //JLPL - 9_23_25 - Need Reset from the Quadrants
assign   QO_reset_Hub_S_W  = POR | race_reset; //JLPL - temporary; //JLPL - 9_23_25 - Need Reset from the Quadrants
assign   QO_reset_Hub_S_E  = POR | race_reset; //JLPL - temporary; //JLPL - 9_23_25 - Need Reset from the Quadrants
`endif //JLSW - endif SIMULATE_CLOCK_RESET



Hub iNhub ( //JL

   .WHO     (QI   [0]),
   .WHO_or  (QI_wr[0]), //JL
   .WHO_rd  (QI_ir[0]), //JL
   .WHO_ck  (QI_ck[0]),
   .WHO_reset  (QI_reset_Hub_N_W),  //JL
   .WHI     (QO   [0]),
   .WHI_wr  (QO_or[0]), //JL
   .WHI_ir  (QO_rd[0]), //JL
   .WHI_ck  (QO_ck[0]),
   .WHI_reset  (QO_reset_Hub_N_W),  //JL

   .EHO     (QI   [1]),
   .EHO_or  (QI_wr[1]), //JL
   .EHO_rd  (QI_ir[1]), //JL
   .EHO_ck  (QI_ck[1]),
   .EHO_reset  (QI_reset_Hub_N_E),  //JL
   .EHI     (QO   [1]),
   .EHI_wr  (QO_or[1]), //JL
   .EHI_ir  (QO_rd[1]), //JL
   .EHI_ck  (QO_ck[1]),
   .EHI_reset  (QO_reset_Hub_N_E),  //JLPL

   .HO      (HO   ),
   .HO_or   (HO_or),
   .HO_rd   (HO_rd),
   .HO_ck   (HO_ck),
   .HO_reset   (HO_reset),          //JL
   .HI      (HI   ),
   .HI_wr   (HI_wr),
   .HI_ir   (HI_ir),
   .HI_ck   (HI_ck),
   .HI_reset   (HI_reset),          //JL
   .ID         (1'b0),              // 0 = This module is the NORTH Hub, 1 = This module is the SOUTH hub //JL
   .reset      (race_reset),        //JL
   .clock      (race_clock)         //JL
   );

Hub iShub ( //JL
   .WHO     (QI   [2]),
   .WHO_or  (QI_wr[2]), //JL
   .WHO_rd  (QI_ir[2]), //JL
   .WHO_ck  (QI_ck[2]),
   .WHO_reset  (QI_reset_Hub_S_W),  //JL
   .WHI     (QO   [2]),
   .WHI_wr  (QO_or[2]), //JL
   .WHI_ir  (QO_rd[2]), //JL
   .WHI_ck  (QO_ck[2]),
   .WHI_reset  (QO_reset_Hub_S_W),  //JL

   .EHO     (QI   [3]),
   .EHO_or  (QI_wr[3]), //JL
   .EHO_rd  (QI_ir[3]), //JL
   .EHO_ck  (QI_ck[3]),
   .EHO_reset  (QI_reset_Hub_S_E),  //JL
   .EHI     (QO   [3]),
   .EHI_wr  (QO_or[3]), //JL
   .EHI_ir  (QO_rd[3]), //JL
   .EHI_ck  (QO_ck[3]),
   .EHI_reset  (QO_reset_Hub_S_E),  //JL

   .HO      (HI   ),
   .HO_or   (HI_wr),                //JLSW
   .HO_rd   (HI_ir),                //JLSW
   .HO_ck   (HI_ck),
   .HO_reset   (HI_reset),          //JL
   .HI      (HO   ),
   .HI_wr   (HO_or),                //JLSW
   .HI_ir   (HO_rd),                //JLSW
   .HI_ck   (HO_ck),
   .HI_reset   (HO_reset),          //JL
   .ID         (1'b1),              // 0 = This module is the NORTH Hub, 1 = This module is the SOUTH hub //JL
   .reset      (race_reset),        //JL
   .clock      (race_clock)         //JL
   );


// TileZero =====================================================================================================================

// Decollate the flagging interface //JL
wire [255:0] flag_out; //JL
wire [255:0] flag_in; //JL

assign flag_in = {FLAGo[3],FLAGo[2],FLAGo[1],FLAGo[0]}; //JL
assign FLAGi[0] = flag_out[63:0]; //JL
assign FLAGi[1] = flag_out[127:64]; //JL
assign FLAGi[2] = flag_out[191:128]; //JL
assign FLAGi[3] = flag_out[255:192]; //JL
wire [255:0] timo_in = {QTIMOo[3],QTIMOo[2],QTIMOo[1],QTIMOo[0]}; //JLPL - 11_11_25

wire [3:0] puf_osc; //JL

assign puf_osc[3:0]  =  QPUFosc[3] | QPUFosc[2] | QPUFosc[1] | QPUFosc[0]; //JL

`ifdef synthesis

   TileZero iTZ (
`else

TileZero #(
   .VROMSIZE(65536),                   // Bytes       Via ROM
   .CODESIZE(16384),                   // Bytes       High-speed
   .DATASIZE(16384),                   // Bytes       High-speed
   .WORKSIZE(65536)                    // Bytes x 2   High-density
   ) iTZ (
`endif
`ifdef SIMULATE_CLOCK_RESET //JLSW
   .clockd     (clockd), 
   .clocki     (clocki), 
   .clocki_fast (clocki_fast), 
   .reset      (reset),
   .imiso(imiso),           //JLPL
   .imosi(imosi),           //JLPL
   .intp_news(intp_news),   //JLPL
   .tz_clk(tz_clk),         //JLPL
   .tz_data(tz_data),       //JLPL
   .tz_fb(tz_fb),           //JLPL
   .rz_clk(rz_clk),         //JLPL
   .rz_data(rz_data),       //JLPL
   .rz_fb(rz_fb),           //JLPL
`endif //JLSW - endif SIMULATE_CLOCK_RESET
   // Output System Interconnect
   .xmiso_clk  (TR_zero_ck ),          // 97       96     95:88     87:80     79:72     71:64     63:32      31:0 //JL
   .xmiso      (TR_zero    ),          // xtmo,    xordy, xack[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xrd0[31:0],xrd1[31:0]} //JL
   .xmiso_fb   (TR_zero_fb ),          //          xpop //JL
   // Input System Interconnect
   .xmosi_clk  (RT_zero_ck ),          // 97       96     95:88     87:80     79:72     71:64     63:32      31:0 //JL
   .xmosi      (RT_zero    ),          // xresetn, xpush, //JL
   .xmosi_fb   (RT_zero_fb ),          //          xirdy //JL
   // NEWS Interfaces
   .wmosi_data (W_txdi     ),          //
   .wmosi_give (W_txwr     ),          //
   .wmosi_take (W_txir     ),          //
   .wmosi_clock(W_txck     ),          //
   .wmosi_reset(W_txrs     ),          //
   .wmosi_clki (W_clki     ),          //JLSW
   .wmiso_data (W_rxdo     ),          //
   .wmiso_give (W_rxor     ),          //
   .wmiso_take (W_rxrd     ),          //
   .wmiso_clock (wmiso_clock),         //JLSW
   .wmiso_reset (wmiso_reset),         //JLSW

   .smosi_data (S_txdi     ),          //
   .smosi_give (S_txwr     ),          //
   .smosi_take (S_txir     ),          //
   .smosi_clock(S_txck     ),          //
   .smosi_reset(S_txrs     ),          //
   .smosi_clki (S_clki     ),          //JLSW
   .smiso_data (S_rxdo     ),          //
   .smiso_give (S_rxor     ),          //
   .smiso_take (S_rxrd     ),          //
   .smiso_clock (smiso_clock),         //JLSW
   .smiso_reset (smiso_reset),         //JLSW

   .emosi_data (E_txdi     ),          //
   .emosi_give (E_txwr     ),          //
   .emosi_take (E_txir     ),          //
   .emosi_clock(E_txck     ),          //
   .emosi_reset(E_txrs     ),          //
   .emosi_clki (E_clki     ),          //JLSW
   .emiso_data (E_rxdo     ),          //
   .emiso_give (E_rxor     ),          //
   .emiso_take (E_rxrd     ),          //
   .emiso_clock (emiso_clock),         //JLSW
   .emiso_reset (emiso_reset),         //JLSW

   .nmosi_data (N_txdi     ),          //
   .nmosi_give (N_txwr     ),          //
   .nmosi_take (N_txir     ),          //
   .nmosi_clock(N_txck     ),          //
   .nmosi_reset(N_txrs     ),          //
   .nmosi_clki (N_clki     ),          //JLSW
   .nmiso_data (N_rxdo     ),          //
   .nmiso_give (N_rxor     ),          //
   .nmiso_take (N_rxrd     ),          //
   .nmiso_clock (nmiso_clock),         //JLSW
   .nmiso_reset (nmiso_reset),         //JLSW

   // PVT Interface
   .pmosi      (pmosi      ),          // 16       clock,enable,psample[1:0],vsample,trimg[4:0],trimo[5:0]} //JL
   .pmiso      (pmiso      ),          // 11       data_valid, data[7:0] } //JL
   // JTAG
   .tdo        (tdo        ),
   .tdi        (tdi        ),
   .tck        (tck        ),
   .tms        (tms        ),
   .trst       (trst       ),
   // Global Interconnect
   .sync_phase (sync_phase ),          //JLPL - 10_23_25
   .race_clock (race_clock ),          // Clock to North Hub of Raceway
   .race_reset (race_reset ),          // Reset to North Hub of Raceway
   .flag_out   (flag_out   ),          // To   Tile[N]
   .flag_in    (flag_in    ),          // From Tile[N]
   .timo_in    (timo_in[255:1]),       // From Tile[N] //JLPL - 11_11_25
//   .timo       (timo       ),        // From Tile[N] //***
   .puf_osc    (puf_osc    ),          //***
   .PoR_bypass (PoR_bypass ),          //*** Power-On Reset bypass
   .PoR_extern (PoR_extern ),          //*** Power-On Reset external when bypassed
   .PoR        (POR        )           // Power-On Reset //JLSW
   );

// PVT Monitor ==================================================================================================================

ABKSGBB2 PVT_ABKSGBB2 ( //JL
   // Output Interface
   .DATA_VALID (pmiso[   10]), //JL
//   .DataOut    (pmiso[ 9:0]), //JL
   .DATA_OUT9  (pmiso[ 9]   ), //JL
   .DATA_OUT8  (pmiso[ 8]   ), //JL
   .DATA_OUT7  (pmiso[ 7]   ), //JL
   .DATA_OUT6  (pmiso[ 6]   ), //JL
   .DATA_OUT5  (pmiso[ 5]   ), //JL
   .DATA_OUT4  (pmiso[ 4]   ), //JL
   .DATA_OUT3  (pmiso[ 3]   ), //JL
   .DATA_OUT2  (pmiso[ 2]   ), //JL
   .DATA_OUT1  (pmiso[ 1]   ), //JL
   .DATA_OUT0  (pmiso[ 0]   ), //JL
   // Input  Interface
   .CLK        (pmosi[   15]),
//   .TRIMG      (pmosi[14:10]), //JL
   .TRIMG4     (pmosi[14]   ), //JL
   .TRIMG3     (pmosi[13]   ), //JL
   .TRIMG2     (pmosi[12]   ), //JL
   .TRIMG1     (pmosi[11]   ), //JL
   .TRIMG0     (pmosi[10]   ), //JL   
//   .TRIMO      (pmosi[ 9: 4]), //JL
   .TRIMO5     (pmosi[ 9]   ), //JL
   .TRIMO4     (pmosi[ 8]   ), //JL
   .TRIMO3     (pmosi[ 7]   ), //JL
   .TRIMO2     (pmosi[ 6]   ), //JL
   .TRIMO1     (pmosi[ 5]   ), //JL
   .TRIMO0     (pmosi[ 4]   ), //JL
   .ENA        (pmosi[    3]), //JL
   .VSAMPLE    (pmosi[    2]), //JL
//   .PSample    (pmosi[ 1: 0]) //JL
   .PSAMPLE1   (pmosi[1]    ), //JL
   .PSAMPLE0   (pmosi[0]    )  //JL
   );

endmodule

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////(//////
