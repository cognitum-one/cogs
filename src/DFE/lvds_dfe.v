//===========================================================
// Karillon Corporation 2021
// Colorado Springs, Colorado, USA
//===========================================================
// Module      : lvds_dfe.v  
// Description : DFE Top
// Author      : Mike Vigil
// Date        : March 21, 2024
// Comments    : Interface clocks 
//             :   - input  clock is clockd
//             :   - output clock is clocko which is buffered clockd
//             : clocki is the original input transmit clock
//             :   - divided by 2 on rising  edge (clocki_div2) for SCO clock
//             :   - divided by 2 on falling edge (clocki_div2_quad) for SDO clocking (tx_clk)
//             : SCI_se is the input serial  rx_clk & was mis-labeled as clocko 
//===========================================================

`ifdef  synthesis //JLSW
   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh"
`else
   `include "A2_project_settings.vh"
`endif

//`timescale 1ps/1fs   // MAV (26May22) -added per Roger's suggestion //JLSW
 
module lvds_dfe #(
   parameter   FWIDTH =  9,
   parameter   FDEPTH =  5  // DEPTH always a power of 2  //. MAV (12Feb25) increased to 5            
   ) (
   // INPUTS
   input wire 	     clockd,// Data System Clock
   input wire 	     clocki,// System Clock 
   input wire 	     reset, // Active High Reset

   input wire [8:0]  TxDataIn, // Transmit Data before 8B10B             // MAV (4Nov24) - increased to 9 bits
   input wire 	     write, // Write (push) DataIn to transmit FIFO
   input wire 	     read, // Read (pop) From Receive FIFO to DataOut
// input wire 	     flush_tfifo, // Flush transmit FIFO                 // MAV (4Nov24) no longer needed
// input wire 	     flush_rfifo, // Flush receive FIFO                  // MAV (4Nov24) no longer needed
// input wire 	     tx_error, // Transmit Control code for Error        // MAV (4Nov24) no longer needed
// input wire 	     tx_break, // Transmit Control code for Break        // MAV (4Nov24) no longer needed
// input wire 	     tx_sof, // Transmit Control code for Start-of-Frame // MAV (4Nov24) no longer needed
// input wire 	     tx_eof, // Transmit Control code for End-of-Frame   // MAV (4Nov24) no longer needed
   input wire 	     SCI_se, // Serial Clock (single-ended) In from LVDS Receiver
   input wire 	     SCI_se_n,// Inverted Serial Clock (single-ended) In from LVDS Receiver
   input wire 	     SDI_se, // Serial Data (single-ended) In from LVDS Receiver
   input wire 	     por, // MAV (10Dec24) added to reset CONFIG 
// OUTPUTS
   output wire [8:0] DataOut, // Receive Data                            // MAV (4Nov24) - increased to 9 bits
   output wire 	     SCO_se, // Serial Clock (single-ended) Out to LVDS Transmitter  // MAV (17Jun22) 
   output wire 	     SDO_se, // Serial Data  (single-ended) Out to LVDS Transmitter
   output wire 	     input_ready, // Transmit FIFO has room for more data
   output wire 	     output_ready, // Receive FIFO has room for more data
// output wire 	     rx_error, // Receive Control code for Error               // MAV (5Nov24) no longer needed
// output wire 	     rx_break, // Receive Control code for Break               // MAV (5Nov24) no longer needed
// output wire 	     rx_sof, // Transmit Control code for Start-of-Frame       // MAV (5Nov24) no longer needed
// output wire 	     rx_eof, // Transmit Control code for End-of-Frame         // MAV (5Nov24) no longer needed
   output wire 	     clocko, // MAV (30Mar25) buffered version of clocki

   // CONFIG
   output wire 	     power_down,
   output wire [5:0] rx_rterm_trim,
   output wire [3:0] rx_offset_trim,
   output wire [4:0] rx_cdac_p,
   output wire [4:0] rx_cdac_m,
   output wire [3:0] rx_bias_trim,
   output wire [3:0] rx_gain_trim,
   output wire [3:0] tx_bias_trim, 
   output wire [3:0] tx_offset_trim,
   output wire [5:0] pmu_bias_trim,

   // SPARE
   output wire [5:0] lvds_spare_in,
   output wire [4:0] analog_test,
   output wire 	     analog_reserved,
   input wire [7:0]  lvds_spare_out, // MAV (8Apr24) - increased from 6 to 8 bits
   output wire 	     preserve_spare_net, // Used to create gates and preserve

   // Single-ended  // MAV (3Jun24)
// input wire 	     SCI_se_bypass, // Serial Clock (single-ended) from GPIO  (17Jan25) replaced with gpio_rxData
// input wire 	     SDI_se_bypass, // Serial Data  (single-ended) from GPIO  (17Jan25) replaced with gpio_rxData

   // MAV (20Jan25) GPIO
   input wire [3:0]  gpio_rxData, // Data from Single-Ended GPIO
   output wire [3:0] gpio_txData, // Data to Single-Ended GPIO
   output wire [3:0] gpio_outEn, // Output Enable to  Single-Ended GPIO 
      
   // MAV (11Mar25) Spare port
   input wire [3:0]  spare_input, 
   output wire [3:0] spare_output   
   );

   
   ////////////////////////// Parameters //////////////////////////
   localparam       SOF        = 8'hFB;
   localparam       EOF        = 8'hFD;
//   localparam       BREAK      = 8'h88;  // DEBUG ONLY
   localparam       BREAK      = 8'hF7;
   localparam       ERROR      = 8'hFE;
   localparam       PAUSE      = 8'h1C; // INTERNAL ONLY
   localparam       RESUME     = 8'h3C; // INTERNAL ONLY
   localparam       IDLE       = 8'hBC; // INTERNAL ONLY
   localparam       RESUMEACK  = 8'h5C; // MAV (23May22)  // INTERNAL ONLY  // (13Feb25) was RDAGAIN
   localparam       PAUSEACK   = 8'h7C; // MAV (13Feb25) added 
   localparam       OVERFLOW   = 8'hFC; // MAV (18Feb) now OVERFLOW
// localparam       CLKDISABLE = 8'h9C; // MAV (29Nov24)  // MAV (4May25) 
   localparam       RESERVED9C = 8'h9C; // MAV (29Nov24)   
   localparam       RESERVEDDC = 8'hDC; // MAV (29Nov24)   
   localparam       CONFIG_WR  = 8'h00; // MAV (1Dec24)
   localparam       CONFIG_RD  = 8'h01; // MAV (1Dec24)
   localparam       STATUS_RD  = 8'h02; // MAV (1Dec24)
// localparam       FLUSH_TX   = 8'h05; // MAV (1Dec24)  // MAV (30Apr25) replaced wiht BREAK
// localparam       FLUSH_RX   = 8'h06; // MAV (1Dec24)  // MAV (30Apr25) replaced wiht BREAK
// localparam       FLUSH_BOTH = 8'h07; // MAV (1Dec24)	 // MAV (30Apr25) replaced wiht BREAK			 
   localparam       DFE_CONNECT  = 8'h80; // MAV (17Jan25)
   localparam       GPIO_CONNECT = 8'h81; // MAV (17Jan25)
   localparam       GPIO_SAMPLE  = 8'h82; // MAV (17Jan25)
   localparam       GPIO_OE      = 4'hA; // MAV (17Jan25)
   localparam       GPIO_DATAOUT = 4'hC; // MAV (17Jan25)
   localparam       GPIO_RDDATA  = 4'hE; // MAV (17Jan25)
   

   ////////////////////////// Regs & Wires //////////////////////////
   reg                 enc_dispin_q, dec_dispin_q;
   reg                 txf_written_once, rxf_written_once;
   wire                txf_write_ored;   // MAV (11Dec24) txf_char_ored no longer needed
   wire [9:0] 	       txf_8b10b_data, rxf_8b10b_data; // 10B Symbol
   wire [8:0] 	       txf_wdata, txf_rdata; // [8] = k_char, [7:0] = data
   wire [8:0] 	       rxf_wdata, rxf_rdata; // [8] = k_char, [7:0] = data
   wire [8:0] 	       txf_rdata_mux; // Mux IDLE if TX_FIFO is empty
   wire                rxf_pause, capture_sipo, load_piso, txf_read, rxf_write, rxf_resume;  // MAV (19Apr22) - added rxf_resume
   wire                enc_dispout, dec_dispout;
   wire 	       txf_resume_ack; // MAV (23May22)	// MAV (13Feb25) was txf_rdAgain      
   wire 	       txf_pause_ack;  // MAV (23May22)	// MAV (13Feb25) added     
   // TX ASYNC FIFO
   wire [FDEPTH:0]     txf_wrWrdCnt, txf_rdWrdCnt;
   wire 	       txf_wrReady,  txf_rdReady;  // MAV (20Apr22) was X_NotEmpty
   // RX ASYNC FIFO
   wire [FDEPTH:0]     rxf_wrWrdCnt, rxf_rdWrdCnt;
   wire 	       rxf_wrReady,  rxf_rdReady; // MAV (20Apr22) was X_NotEmpty		       
   // MOVED COUNTER WITHIN LVDS_DFE
   wire  	       rx_clk, rx_clk_n;   // MAV (30Mar25) was rx_clock_n;
   wire 	       tx_clk_div5, rx_clk_div5 ; // MAV (30Mar25) was clocki_div5; rx_clock_div5
   wire  	       tx_clk, tx_clk_n;

	       
   // MAV (20Jun22)
   reg                 txf_read_once, rxf_read_once;

   // MAV (22Mar24)  - IO Configuration
   reg [63:0] 	       ioConfig;
   wire 	       loopback_Rx2Tx, loopback_internal, loopback_external; 
   wire [3:0]          dfe_spare;
   wire                single_end_bypass, fifo_almost_full_select;  // MAV (12Feb25) added fifo_almost_full_select
   wire                configWrite, configRead, statusRead, configAccess, configReadReady;   // MAV (12Mar25) removed configReset (30Dec24) added configAccess (15Jan25) added configReadReady
   reg [2:0] 	       configState; 
   reg [8:0]           lvdsSpare;         // MAV (8Apr24) - increased from 6 to 8 bits       // MAV (4Nov24) - increased to 9 bits
   wire [8:0] 	       DataOutFifo;       // MAV (8Apr24) - new                              // MAV (4Nov24) - increased to 9 bits
 // MAV (22Dec23) Begin new muxes
   //  Loopback Rx2Tx
   wire [8:0] 	       muxed_txf_wdata;
   wire     	       muxed_txf_write_ored;
// wire 	       muxed_rxf_ready;  // MAV (290Mr25) not used
   //  Loopback Internal
   wire 	       muxed_SCI_se, muxed_SCI_se_n; // MAV (13Dec24) added muxed_SCI_se_n
   wire 	       muxed_SDI_se;
   //  Loopback External
   wire 	       preMuxed_SDO_se;
   wire 	       preMuxed_SCO_se;
   // MAV (4Jun24) Bypass wires
   wire                muxed_SCI_se_bypass, muxed_SCI_se_bypass_n, SCI_se_bypass_n; // MAV (13Dec24) added  muxed_SCI_se_bypass_n & SCI_se_bypass_n
   wire                muxed_SDI_se_bypass; 
   // MAV (4May25)
   wire 	       tx_error1;   // Transmit Control code for Error // MAV (4May25) changed to tx_error1
   wire 	       tx_break1;   // Transmit Control code for Break // MAV (4May25) changed to tx_break1
   wire 	       tx_sof;      // Transmit Control code for Start-of-Frame
   wire 	       tx_eof;      // Transmit Control code for End-of-Frame
   wire 	       tx_res9C, tx_resDC;  // Transmit Control code Reserved bytes
   wire [8:0] 	       rxf_wdata_out; // [8] = k_char, [7:0] = data  // MAV (4May25)
  // MAV (2Dec24) Config Write & Read
   reg [7:0] 	       configRdData;     // Configuration Read
  // MAV (10Dec24) Need to flop for CONFIG state machine
   reg                 configWrite_q, configRead_q, statusRead_q, configRead_q1, statusRead_q1;  // MAV (12Jan25) added configRead_q1 (15Jan25) added statusRead_q1
  // MAV (17Jan25) new wires for GPIO
   wire                gpioConnectLoad, gpioSampleLoad, gpioTxDataLoad,  gpioOutEnableLoad;
   reg 		       gpioConnect_q, gpioSample_q;
   reg [3:0] 	       gpioTxData_q,  gpioOutEnable_q;  // MAV (9Feb25) removed gpioRxData_q;   
   wire[3:0]           gpioRxDataFiltered;  // MAV (24Jan25) added gpioRxDataFiltered (9Feb25) added gpioRxData (17Feb25) removed unused gpioRxData
   reg[3:0]            gpioRxDataFiltered_q;            // MAV (9Feb25) added 
   wire 	       rxf_read_mask;                   // MAV (11Feb25) added to mask RX FIFO Reads   
   // MAV (14Feb25) Overflow
   wire                rxf_write_muxed, rxf_write_overflow;
   wire [8:0] 	       rxf_wdata_muxed; // [8] = k_char, [7:0] = data
   // MAV (1Feb25) ERROR
   wire 	       txf_reset, rxf_reset;  // FIFO async reset
   // MAV (22Mar25) TX FIFO write update
   wire 	       tx_pause, tx_resume, tx_data;  // qualifiers for valid write characters
   wire 	       txf_write_ok;                  // replaces write & ~configAccess
   reg 		       tx_pkt_q;                   // frames between SOF & EOF (1May25) was tx_packet_q
   wire 	       clockdEn, txClkEn, rxClkEn;    // MAV (22Mar25) clock enables   
   wire 	       tx_reserved, tx_resumeAck, tx_pauseAck, tx_overflow;    // MAV (31Mar25) more K-characters
   wire                rxRst, clkiRst, clkdRst; // MAV (6Apr25) synced resets     // MAV (18Apr25) replaced txRst with clkiRst  // 7May25 added clkdRst
   //  MAV (29Apr25) New BREAK ERROR & OVERFLOW logic
   wire 	       tx_break_error_ext, tx_break2, tx_error2;// TX BREAK & ERROR
   reg [2:0]	       tx_cnt_q;                                // Counter for TX break & reset
   wire                rx_break_det, rx_error_det;              // RX BREAK & RESET detected on RX FIFO write data
   wire                rx_overflow_det;                         // RX OVERFLOW detected on RX FIFO write data & extended
   reg [2:0]           rx_cnt_q;                                // Counters for RX Overflow
   reg 		       rx_overflow_det_q;                       // RX OVERFLOW detected flopped version 
   //  MAV (6May25) New Status logic
   wire [7:0] 	       ctlStatus, dfeSpare1, dfeSpare2, dfeSpare3; // MAV (6May25) added DFE Spare  
   //  MAV (6May25) added txf_rdata_mux_q back in for data framing
   reg [8:0] 	       txf_rdata_mux_q;
   reg  	       rxf_write_q;
   //  MAV (26May25) added pause_mask for IDLES
   reg 		       pause_mask;
   
   reg             enable_read = 1'b0; //JLPL - 8_2_25
   wire            masked_txf_read; //JLPL - 8_2_25

   //  MAV (15Jun25) added to delay break/error resets
   reg 		       txf_read_break_error_ext, break_error_write_mask, break_error_reset_q, txf_read_break_error_ext_sync_q;
   wire 	       txf_read_break_error_ext_sync, txf_read_break_error_ext_sync_fall;
   
////////////////////////// Assigns & Muxes//////////////////////////
   assign  configAccess = (configWrite || configWrite_q || configRead || configRead_q || statusRead || statusRead_q || configRead_q1 || statusRead_q1);  // MAV (30Dec24) added (15Jan25) added configRead_q1 // MAV (30Apr25) removed  flush_tfifo and flush_rfifo 
   assign  configReadReady = (configRead_q || statusRead_q);  // MAV (15Jan25) added per meeting with Joel  - later removed configRead + configRead_q1 + statusRead 
   assign  txf_write_ored = ((txf_write_ok && input_ready) || tx_break2 || tx_error2) ; // MAV (22Mar25) simplified logic (4May25) added || tx_break2
   assign  input_ready    = (txf_wrReady && ~loopback_Rx2Tx && ~tx_break_error_ext);   // MAV (25Apr22) // MAV (3May22) added && ~input_ready_mask // MAV (4Jan24) added && ~loopback_Rx2Tx  // MAV (1May25) added ~tx_break
   assign  output_ready   = ((rxf_rdReady || configReadReady || gpioSample_q) && ~loopback_Rx2Tx); // MAV (25Apr22) // MAV (4Jan24) added   && ~loopback_Rx2Tx  (15Jan25) added || configReadReady)  (22Jan25) added || gpioSample_q
   assign  txf_wdata      = (tx_sof)     ? {1'h1, SOF}    :   // MAV (3Dec24) removed _q 
			    (tx_eof)     ? {1'h1, EOF}    :   // MAV (3Dec24) removed _q 
	                    (tx_break2)  ? {1'h1, BREAK}  :   // MAV (3Dec24) removed _q // MAV (4May25) now tx_break2
			    (tx_error2)  ? {1'h1, ERROR}  :   // MAV (19Apr22) // MAV (3Dec24) removed _q  // MAV (4May25) now tx_error2
//			    (tx_clkDisEn)? {1'h1, CLKDISABLE}:// MAV (29Apr25) added  // MAV (4May25) replaced with RESERVED
			    (tx_res9C)   ? {1'h1, RESERVED9C}:// MAV (4May25)  added
			    (tx_resDC)   ? {1'h1, RESERVEDDC}:// MAV (4May25)  added
		                           {1'h0,TxDataIn};   // MAV (2May22)

   assign DataOutFifo = (~rxf_written_once)?  9'h000 : rxf_rdata[8:0]; // MAV (5Nov24) simplified logic


////////////////////////// TX //////////////////////////

   //////// TX FIFO ////////    
   A2_adFIFO  // MAV (4Mar25) added _Rst1   // MAV (16Apr25) removed _Rst1
     #(.WIDTH(FWIDTH), .LG2DEPTH(FDEPTH))
   tx_fifo_inst
   //------------------- WRITE OUT --------------------//
   (.AFFwc                      (txf_wrWrdCnt), // Write Word count
    .AFFir                      (txf_wrReady),  // Write Input Ready (not full)// MAV (20Apr22) was X_NotEmpty
    //------------------- WRITE IN  --------------------//
    .iAFF                       (muxed_txf_wdata),   // Data In  // MAV (22Dec23) added muxed_ // (3Dec24) added _q
    .wAFF                       (muxed_txf_write_ored),   // Data Write (push) // MAV (22Dec23) added muxed_  (3Dec24) added _q
    .wr_clk                     (clockd),       // Write Clock  // MAV (18May22) was clocki
    //------------------- READ OUT --------------------//
    .AFFo                       (txf_rdata),    // Data Out
    .AFFrc                      (txf_rdWrdCnt), // Read word count
    .AFFor                      (txf_rdReady),  // Read Output Ready (not empty) // MAV (20Apr22) was X_NotEmpty
    //------------------- WRITE IN --------------------//
    //.rAFF                       (txf_read),     // ~txf_rd_empty_flag),   // Data Read (pop) // MAV (20Apr22) was X_NotEmpty //JLPL - 8_2_25
    .rAFF                       (masked_txf_read),     // ~txf_rd_empty_flag),   // Data Read (pop) // MAV (20Apr22) was X_NotEmpty //JLPL - 8_2_25
    .rd_clk                     (clocki),       // Read Clock  // MAV (12May) was sco_tx_div5 (23Apr25) was tx_clk_div5 
    //------------------- GLOBAL IN  --------------------//
    .reset                      (txf_reset));   // MAV (17Feb25) was ~reset && ~flush_tfifo) // (25Feb25) was .resetn (~txf_reset))

   assign txf_rdata_mux = 
//   (~txf_written_once)            ? {1'h1, IDLE}     : // MAV (14Apr25) added for IDLE Priority // (21May25) removed
     ((rxf_pause)                   ? {1'h1, PAUSE}    : // MAV (19Apr22)
     (rxf_resume)                   ? {1'h1, RESUME}   : // MAV (19Apr22)
     (txf_resume_ack)               ? {1'h1, RESUMEACK}: // MAV (23May22)  // (13Feb25) was RDAGAIN
     (txf_pause_ack)                ? {1'h1, PAUSEACK} :  // MAV (13Feb25) added
     (~txf_rdReady || pause_mask)   ? {1'h1, IDLE}     :  // MAV (21May25) removed ~txf_read & added pause_mask
 		                      {txf_rdata}   ); 
 
   // Flop dispout for encoder 	   	
   always @ (posedge clocki)  // MAV (18May) was clocki_div5 // MAV (6Apr25) removed posedge reset or posedge tx_clk (19May25) was negedge 
    if (clkiRst) begin          // MAV (6Apr25) was (reset)   txRst
	enc_dispin_q  <= `tCQ 1'h0;
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
     else if (txClkEn && ~tx_clk && ~tx_clk_div5 && preMuxed_SCO_se) begin   // MAV (1Apr25) added clk enable  // (6May25) now txClkEn (19May25) added  ~tx_clk_div5 & preMuxed_SCO_se
	enc_dispin_q  <= `tCQ enc_dispout;
     end
   ////////////////////// END CLOCK GATING //////////////////////

   // Flop TX writen once 	   	
    always @ (posedge clockd)  // MAV (18May22) was clocki_div5  // MAV (16Apr25) removed posedge reset or
     if (reset) begin
	txf_written_once  <= `tCQ 1'h0;
	rxf_read_once     <= `tCQ 1'h0;  // MAV (20Jun22)
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
     else if (clockdEn) begin   // MAV (30Mar25) added if (clockdEn)
	txf_written_once  <= `tCQ (write || txf_written_once); // MAV (2Apr24) added _only // MAV (3 Dec24 replaced write_only with write
	rxf_read_once     <= `tCQ ((rxf_rdReady && read) || rxf_read_once); // MAV (20Jun22)
     end
   ////////////////////// END CLOCK GATING //////////////////////

 
   //////// TX 8B10B Encoder ////////
    encode encode_inst    
      // INPUTS
      (.datain         (txf_rdata_mux_q),   // MAV (25Apr22) added _mux // MAV (23May22) added _q  // MAV (12Dec24) removed _q  // (21May25) added _q
      .dispin          (enc_dispin_q), 
      // OUTPUTS
      .dataout         (txf_8b10b_data), 
      .dispout         (enc_dispout)) ;
    

   //////// TX PISO ////////
   lvds_piso lvds_piso_inst
     // INPUTS
     (.clocki          (clocki),      // MAV (17Apr25) added
      .tx_sclk         (tx_clk),      // MAV (18May22) was sco_tx      clocki_div2_quad
      .tx_sclk_n       (tx_clk_n),    // MAV (18May22) was sco_tx_n   ~clocki_div2_quad
      .txClkEn         (txClkEn),     // MAV (30Mar25) added txClkEn port
      .reset           (clkiRst),     // MAV (17Apr25) was txRst
      .load            (load_piso),
      .pdata           (txf_8b10b_data),
      .preMuxed_SCO_se (preMuxed_SCO_se), // MAV (19May25) added
      // OUTPUTS
     .sdata            (preMuxed_SDO_se));


 ////////////////////////// RX ////////////////////////// 
   //////// RX SIPO ////////
   lvds_sipo lvds_sipo_inst
     // INPUTS
     (.rx_sclk         (rx_clk),
      .rx_sclk_n       (rx_clk_n),   // MAV (21Feb22)
      .rxClkEn         (rxClkEn),    // MAV (31Mar25) added txClkEn port
      .reset           (rxRst),
      .capture         (capture_sipo), 
      .rx_idle_det     (rx_idle_det),  // MAV (18Feb22)
      .sdata           (muxed_SDI_se), // MAV (22Dec23) added muxed_
      // OUTPUTS
      .pdata           (rxf_8b10b_data));
   
  
   // Flop dispout for decoder 	   	
   always @ (posedge rx_clk)  // MAV (18May22) was clocki_div5 // MAV (6Apr25) removed  posedge reset or 
     if (rxRst) begin         // MAV (6Apr25) was (reset)
	dec_dispin_q  <= `tCQ 1'h0;
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
     else if (rxClkEn) begin // MAV (1Apr25) added if (rxClkEn) begin begin
	dec_dispin_q  <= `tCQ dec_dispout;
     end
   ////////////////////// END CLOCK GATING //////////////////////

   
  // Flop RX writen once 	   	
    always @ (posedge rx_clk) // MAV (18May22) was clocko_div5 // MAV (6Apr25) removed posedge reset or 
     if (rxRst) begin         // MAV (6Apr25) was (reset)
	rxf_written_once  <= `tCQ 1'h0;
	txf_read_once     <= `tCQ 1'h0; // MAV (20Jun22)
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
     else if (rxClkEn) begin // MAV (1Apr25) added if (rxClkEn) begin
	rxf_written_once  <= `tCQ (rxf_write || rxf_written_once);
	txf_read_once     <= `tCQ ((txf_read && txf_rdReady) || txf_read_once); // MAV (20Jun22)
     end
   ////////////////////// END CLOCK GATING //////////////////////
   
   
   //////// RX 10B8B Decoder ////////
   decode decode_inst 
     (.datain          (rxf_8b10b_data), 
      .dispin          (dec_dispin_q), 
      .dataout         (rxf_wdata), 
      .dispout         (dec_dispout), 
      .code_err        (rxf_code_err), 
      .disp_err        (rxf_disp_err)) ;
 
  
   assign rxf_read_mask = (gpioSample_q || configRead_q || configRead_q1 || statusRead_q || statusRead_q1);  // MAV (11Feb25) 

   //////// RX FIFO ////////
   A2_adFIFO  // MAV (4Mar25) added _Rst1   // MAV (16Apr25) removed _Rst1
     #(.WIDTH(FWIDTH), .LG2DEPTH(FDEPTH))
   rx_fifo_inst
   //------------------- WRITE OUT --------------------//
   (.AFFwc                  (rxf_wrWrdCnt),  // Write word count
    .AFFir                  (rxf_wrReady),  // Write Input Ready (not full) // MAV (20Apr22) was X_NotEmpty
    //------------------- WRITE IN  --------------------//
    .iAFF                   (rxf_wdata_muxed), // Data In  // MAV (12Dec24) removed  _q   // MAV (15Feb25) added _muxed
    .wAFF                   (rxf_write_muxed), // Data Write (push)                       // MAV (15Feb25) added _muxed
    .wr_clk                 (rx_clk), // Write Clock  // MAV (18May22) was clocko_div5  // MAV (21May25) was rx_clk_div5
    //------------------- READ OUT --------------------//
    .AFFo                   (rxf_rdata),   // Data Out  // MAV (22Dec23) added muxed_
    .AFFrc                  (rxf_rdWrdCnt),  // Read word count
    .AFFor                  (rxf_rdReady),  // Read Output Ready (not empty)// MAV (20Apr22) was X_NotEmpty
    //------------------- READ IN --------------------//
    .rAFF                   (rxf_read && ~rxf_read_mask),   // Data Read (pop) // MAV (20Apr22) was X_NotEmpty // (23Jan25) added || gpioSample_q // (11Feb25) replaced ~gpioSample_q with ~rxf_read_mask
    .rd_clk                 (clockd),     // Read Clock   // MAV (18May22) was clocko
    //------------------- GLOBAL IN  --------------------//
    .reset                  (rxf_reset));   // MAV (17Fev25) was ~reset && ~flush_rfifo && ~rxf_overflow_q  // (25Feb25) was .resetn (rxf_reset))


   
 ////////////////////////// CONTROL //////////////////////////
   //////// CONTROL ////////
   lvds_control 
     #(.WIDTH(FWIDTH), .LG2DEPTH(FDEPTH))
   lvds_control_inst
     // INPUTS
     (.tx_clk                   (tx_clk), // MAV (29Oct24) was clocki
      .rx_clk                   (rx_clk),
      .clocki                   (clocki), // MAV (17Apr25)
      .clockd                   (clockd), // MAV (22May22)
      .rx_clk_div5              (rx_clk_div5),
      .tx_clk_div5              (tx_clk_div5),// MAV (21Feb22)   // MAV (12May) was sco_tx_div5
      .txClkEn                  (txClkEn),     // MAV (30Mar25) added txClkEn port
      .rxClkEn                  (rxClkEn),     // MAV (31Mar25) added txClkEn port
      .clockdEn                 (clockdEn),    // MAV (31Mar25) added clockdEn port
      .reset                    (reset), 
      .rxf_wdata                (rxf_wdata),// MAV (9May22) was RxfData // MAV (12Dec24) removed _q
      .rxf_rdata                (rxf_rdata),  
      .txf_written_once         (txf_written_once), // MAV (25Apr22) added
      .rxf_written_once         (rxf_written_once), // MAV (25Apr22) added
      .txf_read_once            (txf_read_once), // MAV (20Jun22) added
      .rxf_read_once            (rxf_read_once), // MAV (20Jun22) added
      // TX FIFO 
      .txf_wrWrdCnt             (txf_wrWrdCnt), // MAV (20Apr22) Replaced previous input flags
      .txf_wrReady              (txf_wrReady),  // MAV (20Apr22) Replaced previous input flags 
      .txf_rdWrdCnt             (txf_rdWrdCnt), // MAV (20Apr22) Replaced previous input flags
      .txf_rdReady              (txf_rdReady),  // MAV (20Apr22) Replaced previous input flags
      // RX FIFO 
      .rxf_wrWrdCnt             (rxf_wrWrdCnt), // MAV (20Apr22) Replaced previous input flags
      .rxf_wrReady              (rxf_wrReady),  // MAV (20Apr22) Replaced previous input flags
      .rxf_rdWrdCnt             (rxf_rdWrdCnt), // MAV (20Apr22) Replaced previous input flags
      .rxf_rdReady              (rxf_rdReady),  // MAV (20Apr22) Replaced previous input flags
      .read                     (read),  // MAV (9May22)
      // SIPO
      .rx_idle_det              (rx_idle_det),  // MAV (18Feb22)
      //OUTPUTS
      .rx_error_det             (rx_error_det),// MAV (4May25) was rxf_error
      .rx_break_det             (rx_break_det),// MAV (4May25) was rxf_break
      .rx_overflow_det          (rx_overflow_det),// MAV (5May25) RX FIFO Overflow moved from lvds_dfe to lvds_control
//    .rxf_sof                  (rxf_sof),     // MAV (4May25) no longer used
//    .rxf_eof                  (rxf_eof),     // MAV (4May25) no longer used
      .rxf_wdata_out            (rxf_wdata_out), // MAV (4May25) RX FIFO write Data Muxed 
      .rxf_pause                (rxf_pause),   // MAV (8Jan25) added _sync
      .rxf_resume               (rxf_resume),  // MAV (8Jan25) added _sync // MAV (19Apr22) - added rxf_resume
      .capture                  (capture_sipo),
      .load                     (load_piso),
      .txf_read                 (txf_read),
      .rxf_write                (rxf_write),  
      .rxf_read                 (rxf_read), // MAV (12May22)
      .txf_resume_ack           (txf_resume_ack),  // MAV (23May22) // MAV (13Feb25) renamed to txf_resume_ack
      .txf_pause_ack            (txf_pause_ack),   // MAV (13Feb25) added
      .fifo_almost_full_select  (fifo_almost_full_select),  // MAV (12Feb25) 
   /// MAV (6Apr25) 
      .clkiRst                  (clkiRst),// MAV (6Apr24) RESET synced to clocki
//    .txRst                    (txRst),  // MAV (6Apr24) RESET synced to TxClk (18Apr25) replaced with clkiRst
      .rxRst                    (rxRst), // MAV (6Apr24) RESET synced to RxClk               
      .ctlStatus                (ctlStatus), // MAV (6May25) added control signals for visabilty
      .preMuxed_SCO_se (preMuxed_SCO_se));   // MAV (19May25) added

   lvds_clock  lvds_clock_inst
     // INPUTS
     (.reset                   (reset),       // reset IN from LVDS core
      .clockd                  (clockd),      // MAV (6Apr25) 
      .clocki                  (clocki),      // System TX Clock    // MAV (9Apr25) - need for TX Reset sync
//    .clocki_div2_quad        (clocki_div2_quad),  // TX Div2 & quadrature Clock  (6Apr25) moved logic into lvds_clock
      .SCI_se                  (muxed_SCI_se),// Same as clocko  // MAV (22Dec23) added muxed_
      .SCI_se_n                (muxed_SCI_se_n),// Inverted muxed_SCI_se
//    .SDI_se                  (muxed_SDI_se),// MAV (30Mar24) added input
      .rx_idle_det             (rx_idle_det), // MAV (2Apr24) used to re-sync clocko_div5 
      .txClkEn                 (txClkEn),     // MAV (30Mar25) added txClkEn port
      .rxClkEn                 (rxClkEn),     // MAV (31Mar25) added txClkEn port
      // OUTPUTS
      .rx_clk                  (rx_clk),      // Buffered version of SCI_se
      .rx_clk_n                (rx_clk_n),    // Buffered & inverted version of SCI_se
      .rx_clk_div5             (rx_clk_div5), // Divided Parallel Clock Out to LVDS core 
      .rx_clk_early_div5       (rx_clk_early_div5), // Early Divided Parallel Clock Out to LVDS core MAV (23May25) for RX FIFDO write_q
      .tx_clk                  (tx_clk),  // Buffered version of clocki   // MAV (6Apr25) removed _buf
      .tx_clk_n                (tx_clk_n),// Buffered & inverted version of clocki // MAV (6Apr25) removed _buf
      .tx_clk_div5             (tx_clk_div5), // Divided Parallel Clock to Out Digital
   /// MAV (6Apr25) 
      .clkiRst                 (clkiRst),// MAV (6Apr24) RESET synced to clocki  
//    .txRst                   (txRst),  // MAV (6Apr24) RESET synced to TxClk  (18Apr25) replaced with clkiRst
      .rxRst                   (rxRst),  // MAV (6Apr24) RESET synced to RxClk
      .clocko                  (clocko),           // MAV (7Apr25) moved to lvds_clock
      .preMuxed_SCO_se         (preMuxed_SCO_se)); // MAV (7Apr25) replaced clocki_div2
   

       
// MAV (22Dec23) Begin new muxes
   //  Loopback Rx2Tx
   assign muxed_txf_wdata      = (loopback_Rx2Tx)? rxf_rdata   : txf_wdata;
   assign muxed_txf_write_ored = (loopback_Rx2Tx)? rxf_rdReady : txf_write_ored;
   assign muxed_rxf_read       = (loopback_Rx2Tx)? txf_wrReady : rxf_read; 
   //  LoopbackInternal
   assign muxed_SCI_se         = (loopback_internal)?  preMuxed_SCO_se : muxed_SCI_se_bypass;  // MAV (4Jun24) was SCI_se  
   assign muxed_SDI_se         = (loopback_internal)?  preMuxed_SDO_se : muxed_SDI_se_bypass;  // MAV (4Jun24) was SDI_se  
   assign muxed_SCI_se_n       = (loopback_internal)? ~preMuxed_SCO_se : muxed_SCI_se_bypass_n;// MAV (13Dec24) added new mux (7Apr25) was clocki_div2_n
   //  Loopback External
   assign SDO_se               = (loopback_external)?  muxed_SDI_se_bypass : preMuxed_SDO_se;  // MAV (4Jun24) was SDI_se  
   assign SCO_se               = (loopback_external)?  muxed_SCI_se_bypass : preMuxed_SCO_se;  // MAV (4Jun24) was SCI_se  



   // MAV - Configuration
   assign  configWrite = (write && (TxDataIn == {1'h1,CONFIG_WR})); // MAV (2Dec24) was write & tx_sof & tx_eof; 
   assign  configRead  = (write && (TxDataIn == {1'h1,CONFIG_RD})); // MAV (2Dec24) added 
   assign  statusRead  = (write && (TxDataIn == {1'h1,STATUS_RD})); // MAV (2Dec24) added 
// assign  configReset = (reset && por); // MAV (12Mar25) reverted from configReset back to reset 


  // Flops for state machine 
  always @ (posedge clockd) // MAV (6Apr25) posedge reset or 
    if (clkdRst) begin       // MAV (7May25) was (reset)
      configWrite_q  <= `tCQ 1'h0;
   end
   else if (write && configState == 3'h7) begin   // MAV (12Jan25) was configWrite_q
      configWrite_q <= `tCQ 1'h0;
   end
   else if (configWrite || configWrite_q)  begin
      configWrite_q <= `tCQ 1'h1;
   end

   else  begin
      configWrite_q <= `tCQ configWrite;
   end
      
   always @ (posedge clockd)  // MAV (6Apr25) removed posedge reset or
    if (clkdRst) begin       // MAV (7May25) was (reset)
	configRead_q  <= `tCQ 1'h0;
     end
     else if (configRead)  begin
	configRead_q <= `tCQ 1'h1;
     end
     else if (configRead_q && configState == 3'h7 && read ) begin   // MAV (15Jan25) added && read
	configRead_q <= `tCQ 1'h0;
     end

   // MAV (12Jan25) need one more aserted read for last byte
   always @ (posedge clockd)  // MAV (6Apr25) removed posedge reset or 
     if (clkdRst) begin	   // MAV (7May25) was (reset)
	configRead_q1  <= `tCQ 1'h0;
	statusRead_q1  <= `tCQ 1'h0;
     end
     else begin
	configRead_q1  <= `tCQ configRead_q;
	statusRead_q1  <= `tCQ statusRead_q;
     end


   
  always @ (posedge clockd) // MAV (6Apr25) removed posedge reset or 
    if (clkdRst) begin       // MAV (7May25) was (reset)
      statusRead_q  <= `tCQ 1'h0;
   end
   else if (statusRead)  begin
      statusRead_q <= `tCQ 1'h1;
   end else if (statusRead_q && configState == 3'h3 && read) begin  // MAV (15Jan25) was  end else  begin
      statusRead_q <= `tCQ 1'h0;
   end

   // RESET [63:0]     0x00000_01CC_CCCE_718E	
   always @ (posedge clockd) // MAV (12Mar25) reverted from configReset back to reset // MAV (6Apr25) removed posedge reset or 
     if (reset) begin                      // MAV (12Mar25) reverted from configReset back to reset
	configState     <= `tCQ 3'd0;        // Config state	
	ioConfig[63:56] <= `tCQ 8'b00000000; // DFE
	ioConfig[   55] <= `tCQ 6'b000000;   // AFE Reserved
	ioConfig[54:50] <= `tCQ 5'b00000;    // AFE Test
	ioConfig[49:44] <= `tCQ 6'b000000;   // AFE Spare  49 & 48 now disable
	ioConfig[43:38] <= `tCQ 6'b000111;   // PMU Bias Trim	
	ioConfig[37:34] <= `tCQ 4'b0011;     // TX Amp Offset Voltage Trim	
	ioConfig[33:30] <= `tCQ 4'b00011;    // TX Amp Bias Current Trim
	ioConfig[29:26] <= `tCQ 4'b0011;     // Rx Gain Amp Gain Trim	
	ioConfig[25:22] <= `tCQ 4'b0011;     // Rx Gain Amp Bias Current Trim		
	ioConfig[21:17] <= `tCQ 5'b00111;    // Rx Negative CDAC Delay Trim
	ioConfig[16:12] <= `tCQ 5'b00111;    // Rx Positive CDAC Delay Trim
	ioConfig[11: 7] <= `tCQ 4'b011;      // Rx Offset Resistor Trim
	ioConfig[ 6: 1] <= `tCQ 6'b000111;   // Rx Termination Resistor Trim
	ioConfig[    0] <= `tCQ 1'b0;        // Powerdown
//      configRdData    <= `tCQ 8'h0;        // Config read data	// MAV (28May25) moved to end 
//     end else if (configState == 3'd7) begin // MAV (9Jan25) need tp clear ConfigState
//	   configState   <= `tCQ 3'd0;	

     end else if (configWrite_q && write) begin // MAV (3Dec24) added to split write & read (8Jan24) added  && write
	if (configState == 3'd0) begin 
	   ioConfig[7:0] <= `tCQ TxDataIn[7:0];
	   configState   <= `tCQ 3'd1;
	end else if (configState == 3'd1) begin 
	   ioConfig[15:8] <= `tCQ TxDataIn[7:0];
	   configState    <= `tCQ 3'd2;
	end else if (configState == 3'd2) begin 
	   ioConfig[23:16] <= `tCQ TxDataIn[7:0];
	   configState     <= `tCQ 3'd3;
	end else if (configState == 3'd3) begin 
	   ioConfig[31:24] <= `tCQ TxDataIn[7:0];
	   configState     <= `tCQ 3'd4;
	end else if (configState == 3'd4) begin 
	   ioConfig[39:32] <= `tCQ TxDataIn[7:0];
	   configState     <= `tCQ 3'd5;
	end else if (configState == 3'd5) begin 
	   ioConfig[47:40] <= `tCQ TxDataIn[7:0];
	   configState     <= `tCQ 3'd6;
	end else if (configState == 3'd6) begin 
	   ioConfig[55:48] <= `tCQ TxDataIn[7:0];
	   configState     <= `tCQ 3'd7;
	end else /*if (configState == 3'd7)*/ begin // MAV (5Nov25) Fix for synthesis
	   ioConfig[63:56] <= `tCQ TxDataIn[7:0];
	   configState     <= `tCQ 3'd0;
	end
     end // if (configWrite)

     // MAV (3Dec24) added config read section
     else if (configRead_q && read ) begin   // MAV (15Jan25) added && read per meeting with Joel
	if (configState == 3'd0) begin 
	 //configRdData[7:0] <= `tCQ ioConfig[7:0]; // MAV (15Jan25) moved to end
	   configState       <= `tCQ 3'd1;
	end else if (configState == 3'd1) begin 
	 //configRdData[7:0] <= `tCQ ioConfig[15:8];// MAV (15Jan25) moved to end
	   configState       <= `tCQ 3'd2;
	end else if (configState == 3'd2) begin 
	 //configRdData[7:0] <= `tCQ ioConfig[23:16];// MAV (15Jan25) moved to end
	   configState       <= `tCQ 3'd3;
	end else if (configState == 3'd3) begin 
	 //configRdData[7:0] <= `tCQ ioConfig[31:24];// MAV (15Jan25) moved to end
	   configState       <= `tCQ 3'd4;
	end else if (configState == 3'd4) begin 
	 //configRdData[7:0] <= `tCQ ioConfig[39:32];// MAV (15Jan25) moved to end
	   configState       <= `tCQ 3'd5;
	end else if (configState == 3'd5) begin 
	 //configRdData[7:0] <= `tCQ ioConfig[47:40];
	   configState       <= `tCQ 3'd6;
	end else if (configState == 3'd6) begin 
	 //configRdData[7:0] <= `tCQ ioConfig[55:48];// MAV (15Jan25) moved to end
	   configState       <= `tCQ 3'd7;
	end else /*if (configState == 3'd7)*/ begin  // MAV (5Nov25) Fix for synthesis
	 //configRdData[7:0] <= `tCQ ioConfig[63:56];// MAV (15Jan25) moved to end
	   configState       <= `tCQ 3'd0;
	end
     end // if (configRead)

     // MAV (15Jan25) added statys read section
     else if (statusRead_q && read ) begin   // MAV (15Jan25) added && read per meeting with Joel
	if (configState == 3'd0) begin 
	   configState       <= `tCQ 3'd1;
	end else if (configState == 3'd1) begin 
	   configState       <= `tCQ 3'd2;
	end else if (configState == 3'd2) begin 
	   configState       <= `tCQ 3'd3;
	end else /*if (configState == 3'd3)*/ begin  // MAV (5Nov25) Fix for synthesis
	   configState       <= `tCQ 3'd0;
	end
     end // if (statusRead)
	
	
   
       
   // MAV AFE bit assignments - (9Feb25) reorder from MSB to LSB 
   assign single_end_bypass = ioConfig[63];
   assign dfe_spare         = ioConfig[62:59];  // MAV (30Dec24) was [62:60] (12Feb25) [62] selects almost_full value
   assign loopback_external = ioConfig[58];     // MAV (30Dec24) was [59]
   assign loopback_internal = ioConfig[57];     // MAV (30Dec24) was [58]
   assign loopback_Rx2Tx    = ioConfig[56];     // MAV (30Dec24) was [57]
   assign analog_reserved   = ioConfig[55];
   assign analog_test       = ioConfig[54:50];
   assign lvds_spare_in     = ioConfig[49:44]; // MAV (9Feb25) [49] = Rx Power Down [48] = Tx Power Down
   assign pmu_bias_trim     = ioConfig[43:38];
   assign tx_offset_trim    = ioConfig[37:34];
   assign tx_bias_trim      = ioConfig[33:30];  
   assign rx_gain_trim      = ioConfig[29:26];
   assign rx_bias_trim      = ioConfig[25:22];
   assign rx_cdac_m         = ioConfig[21:17];
   assign rx_cdac_p         = ioConfig[16:12];
   assign rx_offset_trim    = ioConfig[11:7];
   assign rx_rterm_trim     = ioConfig[6:1];
   assign power_down        = ioConfig[0]; // MAV (4Jun24) added OR  || ioConfig[63]  // 9Feb25 removed || ioConfig[63]
   assign fifo_almost_full_select = dfe_spare[3];  // MAV (12Feb25) added

   // MAV - Spare AFE // MAV (6May25) added DFE Spares	   	
   assign dfeSpare1 = {rx_cnt_q[2:0],ctlStatus[4:0]};   // RX LOGIC
   assign dfeSpare2 = {3'b0, tx_break_error_ext,        // TX LOGIC
		       tx_pkt_q,tx_cnt_q[2:0]};
   assign dfeSpare3 = {1'b0,configWrite_q,configRead_q,statusRead_q,  // CONFIG LOGIC
		       1'b0,configState[2:0]};

   // MAV - Spare AFE 	   	
   always @ (posedge clockd) // MAV (12Mar25) reverted from configReset back to reset // MAV (6Apr25) removed posedge reset or 
     if (clkdRst) begin	   // MAV (7May25) was (reset)
   	lvdsSpare  <= `tCQ 8'b0;	
//     end else begin
//	lvdsSpare  <= `tCQ lvds_spare_out; 
//     end
     end else if (configState == 2'b01) begin
	lvdsSpare <= `tCQ dfeSpare1;
     end else if (configState == 2'b10) begin
	lvdsSpare <= `tCQ dfeSpare2;
     end else if (configState == 2'b11) begin
	lvdsSpare   <= `tCQ dfeSpare3;
     end else begin
	lvdsSpare <= `tCQ lvds_spare_out;
     end

   
   // MAV - generate spare cells from spare flops
   wire preserve_spare_dfe =  
 	 ( dfe_spare[0] &  dfe_spare[1]) ||
         ( dfe_spare[1] |  dfe_spare[2]) ||
         (~dfe_spare[2] ^ ~dfe_spare[3]) ||
        ~( dfe_spare[0] &  dfe_spare[1]) ||
        ~( dfe_spare[1] |  dfe_spare[2]) ||
        ~(~dfe_spare[2] ^ ~dfe_spare[3]);
     
   wire preserve_spare_afe =  
 	 (lvdsSpare[0]  | lvdsSpare[1]) && 
	 (lvdsSpare[1]  & lvdsSpare[2]) && 
	 (lvdsSpare[2]  ^ lvdsSpare[3]) && 
	 (lvdsSpare[3]  | lvdsSpare[4]) &&
	~(lvdsSpare[4]  & lvdsSpare[5]) &&
	~(lvdsSpare[5]  ^ lvdsSpare[6]) &&  
	~(lvdsSpare[6]  | lvdsSpare[7]) && // MAV (8Apr24) added 
	~(lvdsSpare[7]  & lvdsSpare[0]);   // MAV (8Apr24) added 
   
   wire preserve_spare_mux0 = (lvdsSpare[1])?  preserve_spare_dfe :  preserve_spare_afe;
   wire preserve_spare_mux1 = (lvdsSpare[2])?  preserve_spare_dfe : ~preserve_spare_afe;
   wire preserve_spare_mux2 = (lvdsSpare[3])? ~preserve_spare_dfe :  preserve_spare_afe;   
   wire preserve_spare_mux3 = (lvdsSpare[4])? ~preserve_spare_dfe : ~preserve_spare_afe;

   assign preserve_spare_net   = (~preserve_spare_dfe  & ~preserve_spare_afe  & ~preserve_spare_mux0) ~^
			       (~preserve_spare_mux1 | ~preserve_spare_mux2 | ~preserve_spare_mux3);
   
   assign DataOut = (configRead_q || configRead_q1)? {1'b0, configRdData}: // MAV (2Dec24) added {1'b0, ...} ? configRdData:  // (12Jan25) added || configRead_q1  (11Feb25) removed read &&
                    (statusRead_q || statusRead_q1)? {1'b0, lvdsSpare}  : // MAV (15Jan25) added ||  stausRead_q1 (11Feb25) removed read &&
                    (gpioSample_q)                 ? {1'b1, GPIO_RDDATA, gpioRxDataFiltered_q} : // MAV (22Jan25) (11Feb25) removed read &&
			                             DataOutFifo; 

// MAV (4Jun24) Single-ended muxes
   assign  SCI_se_bypass         = gpio_rxData[3];  // MAV (20Jan25)
   assign  SDI_se_bypass         = gpio_rxData[2];  // MAV (20Jan25)
   assign  muxed_SCI_se_bypass   = (single_end_bypass)? SCI_se_bypass   : SCI_se;
   assign  muxed_SDI_se_bypass   = (single_end_bypass)? SDI_se_bypass   : SDI_se;
   assign  muxed_SCI_se_bypass_n = (single_end_bypass)? SCI_se_bypass_n : SCI_se_n; // MAV (13Dec24) added new mux

// MAV (6Apr25) Moved clocki_div2 & clocki_div2_quad to lvds_clock
   
// MAV (2Dev24) new muxing logic // MAV (9Apr25) new sync reset causing Xs because TxDataIn Xs until write 

   assign tx_error1    = (~write) ? 1'b0 : ((TxDataIn == {1'h1,ERROR}) && ~break_error_write_mask) ? 1'b1 : 1'b0; // MAV (4May25) changed to tx_error1 & added && (tx_cnt_q  < 3'h2)  
   assign tx_break1    = (~write) ? 1'b0 : ((TxDataIn == {1'h1,BREAK}) && ~break_error_write_mask) ? 1'b1 : 1'b0; // MAV (4May25) changed to tx_break1 & added && (tx_cnt_q  < 3'h2) (tx_cnt_q < 3'h3)
   assign tx_sof       = (~write) ? 1'b0 : (TxDataIn == {1'h1,SOF})   ? 1'b1 : 1'b0;	
   assign tx_eof       = (~write) ? 1'b0 : (TxDataIn == {1'h1,EOF})   ? 1'b1 : 1'b0;
// assign tx_clkDisEn  = (~write) ? 1'b0 : (TxDataIn == {1'h1,CLKDISABLE}) ? 1'b1 : 1'b0; // MAV (4May25) replaced with tx_Res9C
   assign tx_res9C     = (~write) ? 1'b0 : (((TxDataIn == {1'h1,RESERVED9C}) ? 1'b1 : 1'b0)); // MAV (4May25) replaced tx_clkDisEn	
   assign tx_resDC     = (~write) ? 1'b0 : (((TxDataIn == {1'h1,RESERVEDDC}) ? 1'b1 : 1'b0)); // MAV (4May25) added	
   assign tx_break2    = ((tx_cnt_q == 3'h5) && TxDataIn == {1'h1,BREAK})   ? 1'b1 : 1'b0;   // MAV (4May25) added
   assign tx_error2    = ((tx_cnt_q == 3'h5) && TxDataIn == {1'h1,ERROR})   ? 1'b1 : 1'b0;   // MAV (4May25) added
   
  // MAV (13May24) added inverters for single-ended bypass muxing
   assign SCI_se_bypass_n = ~SCI_se_bypass;


   // MAV (15Jan25) moved configRdData to here
   always @ (*) begin
      case (configState) 
	3'h1 :   configRdData[7:0] = ioConfig[15:8];  // changed from <= `tCQ
	3'h2 :   configRdData[7:0] = ioConfig[23:16]; // changed from <= `tCQ
	3'h3 :   configRdData[7:0] = ioConfig[31:24]; // changed from <= `tCQ
	3'h4 :   configRdData[7:0] = ioConfig[39:32]; // changed from <= `tCQ
	3'h5 :   configRdData[7:0] = ioConfig[47:40]; // changed from <= `tCQ
	3'h6 :   configRdData[7:0] = ioConfig[55:48]; // changed from <= `tCQ
	3'h7 :   configRdData[7:0] = ioConfig[63:56]; // changed from <= `tCQ
	default: configRdData[7:0] = ioConfig[7:0];   // changed from <= `tCQ
      endcase // case (configState)
   end


  // MAV (17Jan25) new logic for GPIO
   assign  gpioConnectLoad   = (write && (TxDataIn      == {1'h1,GPIO_CONNECT}));   // 0 = DFE 1 = GPIO
   assign  dfeConnectLoad    = (write && (TxDataIn      == {1'h1,DFE_CONNECT}));    // 0 = DFE 1 = GPIO
   assign  gpioSampleLoad    = (write && (TxDataIn      == {1'h1,GPIO_SAMPLE}));  
   assign  gpioOutEnableLoad = (write && (TxDataIn[8:4] == {1'h1,GPIO_OE}));  
   assign  gpioTxDataLoad    = (write && (TxDataIn[8:4] == {1'h1,GPIO_DATAOUT}));  



   always @ (posedge clockd)  // MAV (6Apr25) removed posedge reset or
     if (reset) begin
	gpioConnect_q   <= `tCQ 1'h0;
     end else if (gpioConnectLoad) begin
	gpioConnect_q   <= `tCQ  1'b1;
     end else if (dfeConnectLoad) begin
	gpioConnect_q   <= `tCQ  1'b0;
     end

   always @ (posedge clockd)  // MAV (6Apr25) removed posedge reset or
     if (reset) begin
	gpioSample_q    <= `tCQ 1'h0;		    
    end else if (gpioSampleLoad) begin
	gpioSample_q    <= `tCQ 1'h1;		    
    end else if (read) begin
	gpioSample_q    <= `tCQ 1'h0;
    end

   always @ (posedge clockd)   // MAV (6Apr25) removed posedge reset or
     if (reset) begin
	gpioOutEnable_q <= `tCQ 4'h0;
     end else if (gpioOutEnableLoad) begin
	gpioOutEnable_q <= `tCQ TxDataIn[3:0];
     end

   always @ (posedge clockd)   // MAV (6Apr25) removed posedge reset or
     if (reset) begin
	gpioTxData_q    <= `tCQ 4'h0;	    	    
     end else if (gpioTxDataLoad) begin
	gpioTxData_q    <= `tCQ TxDataIn[3:0];
     end

   always @ (posedge clockd)   // MAV (6Apr25) removed posedge reset or
     if (reset) begin
	gpioRxDataFiltered_q  <= `tCQ 4'h0;	         // MAV (9Feb25) was gpioRxData_q	    
     end else if (gpioSampleLoad) begin
	gpioRxDataFiltered_q  <= `tCQ gpioRxDataFiltered; // MAV (9Feb25) was gpioRxData_q  <= #10 gpioRxData[3:0]
     end

   assign  gpio_outEn  = (gpioConnect_q)? gpioOutEnable_q : {1'b0, 1'b0, 1'b1, 1'b1}; // SxI  = IN, SxO = OUT
   assign  gpio_txData = (gpioConnect_q)? gpioTxData_q    : {2'b0, SCO_se, SDO_se};   // Only SDO/SCO need muxing
			    

// MAV (24Jan25) added filters
A2_digital_filter  gpioRxDataFilter_inst3 ( .level (gpioRxDataFiltered[3]), .source(gpio_rxData[3]), .clock (clockd), .reset (reset)); // MAV (9Feb25) was gpioRxData_q & ~reset (17Feb25) was (gpioRxData[3])
A2_digital_filter  gpioRxDataFilter_inst2 ( .level (gpioRxDataFiltered[2]), .source(gpio_rxData[2]), .clock (clockd), .reset (reset)); // MAV (9Feb25) was gpioRxData_q & ~reset (17Feb25) was (gpioRxData[2])
A2_digital_filter  gpioRxDataFilter_inst1 ( .level (gpioRxDataFiltered[1]), .source(gpio_rxData[1]), .clock (clockd), .reset (reset)); // MAV (9Feb25) was gpioRxData_q & ~reset (17Feb25) was (gpioRxData[1])
A2_digital_filter  gpioRxDataFilter_inst0 ( .level (gpioRxDataFiltered[0]), .source(gpio_rxData[0]), .clock (clockd), .reset (reset)); // MAV (9Feb25) was gpioRxData_q & ~reset (17Feb25) was (gpioRxData[0])
   
// moved OVERFLOW to end

// FIFO Aysnchronous Resets
   assign txf_error_reset = (txf_read)? 1'b0 : (txf_rdata_mux == 9'h1FE);      
   assign rxf_error_reset = (read)?     1'b0 : (DataOut == 9'h1FE) || (rxf_wrWrdCnt == ((2 ** FDEPTH)-1));   // MAV (21Apr25)
   assign txf_reset = (reset || break_error_reset_q);  // MAV (18Apr25) replaced txf_error_reset_q1  // MAV (4May25) added tx_break1 || tx_reset1 (15Jun25) replaced  tx_break1 || tx_error1
   assign rxf_reset = (reset || rx_break_det || rx_error_det || rx_overflow_det); // MAV (4May25) replaced  rxf_error_reset_q1/rxf_overflow_q* with rx__*_det

  
   // MAV (11Mar25) 
   assign spare_output = spare_input;

   always @ (posedge clockd) // MAV (6Apr25) removed posedge reset or
     if (reset) begin
	tx_pkt_q <= `tCQ 1'h0; // MAV  (1May25) was tx_packet_q
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
     else if (clockdEn) begin       // MAV (1Apr25) added if (clockdEn) 
	if (write && tx_sof) begin  // MAV (1Apr25) removed else 
	   tx_pkt_q <= `tCQ 'h1;      // MAV (1May25) was tx_packet_q
	end
	else if (write && (tx_eof || tx_error1 || tx_break1)) begin    // MAV (2May25) added || tx_error1 || tx_break1
	   tx_pkt_q <= `tCQ 'h0;     // MAV (1May25) was tx_packet_q
	end    
     end  // else if (clockdEn// MAV (1Apr25) added if (clockdEn) 
   ////////////////////// END CLOCK GATING //////////////////////
   

   assign tx_data      = (write && ~TxDataIn[8] && tx_pkt_q);  // non-k characater  // MAV (1May25) was tx_packet_q	 
   assign tx_pause     = (write && (TxDataIn == {1'h1,PAUSE}));   // MAV (31Mar25) removed && tx_pkt_q
   assign tx_resume    = (write && (TxDataIn == {1'h1,RESUME}));  // MAV (31Mar25) removed && tx_pkt_q 	
   assign tx_pauseAck  = (write && (TxDataIn == {1'h1,PAUSEACK})); 
   assign tx_resumeAck = (write && (TxDataIn == {1'h1,RESUMEACK}));
   assign tx_overflow  = (write && (TxDataIn == {1'h1,OVERFLOW}));
   assign tx_reserved  = (write && ((TxDataIn == {1'h1,RESERVED9C}) || (TxDataIn == {1'h1,RESERVEDDC}))); // MAV (31Mar25) added 
   assign txf_write_ok = (tx_sof || (tx_eof && tx_pkt_q) || tx_pause || tx_resume || tx_data ||  // MAV (4May25) added  && tx_pkt_q 
			  tx_pauseAck || tx_resumeAck || tx_overflow || tx_reserved );  // MAV (31Mar25) added  // MAV (4May25) removed tx_clkDisEn

// MAV (22Mar25) clock enables   power_down = pmu_pd ioConfig[49] = Rx Power Down ioConfig[48] = Tx Power Down
   assign clockdEn =  1'b1;                         // MAV (1May25)  clockEn tied off to logic 1
   assign txClkEn  = ~dfe_spare[2];   // MAV (29Apr25) was ~power_down (29May25) was ~ioConfig[48]
   assign rxClkEn  = ~dfe_spare[1];   // MAV (29Apr25) was ~power_down (29May25) was ~ioConfig[49]


// MAV (29Apr25) Counters  // MAV (4May25) updated  BREAK & ERROR logic 
   always @ (posedge clockd) 
     if (reset) begin	
	tx_cnt_q <= `tCQ 3'h0;      
     end
     else if ((tx_break1 || tx_cnt_q > 3'h0) || (tx_error1 || tx_cnt_q > 3'h0)) begin 
 	 tx_cnt_q <= `tCQ tx_cnt_q + 3'h1;
     end

   assign tx_break_error_ext = ((tx_cnt_q > 3'h0 && tx_cnt_q < 3'h6));


////////////////// OVERFLOW //////////////////   
// MAV (4May25) updated OVERFLOW
   assign rxf_write_muxed  = (rxf_write_overflow)?   1'b1            :  rxf_write_q;    // MAV (21May25) added _q
   assign rxf_wdata_muxed  = (rxf_write_overflow)?  {1'b1, OVERFLOW} :  rxf_wdata_out;  // MAV (18Feb25) ERROR - now OVERFLOW // MAV (4May25) added _out


  // RX Counter for OVERFLOW
  always @ (posedge rx_clk)
     if (rxRst) begin 
	rx_cnt_q <= `tCQ 3'h0;      
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
    else if (rxClkEn && ~rx_clk_div5 && (rx_overflow_det_q || rx_cnt_q > 3'h0)) begin
       rx_cnt_q  <= `tCQ rx_cnt_q + 3'h1;
    end //  else if (rxClkEn) 
   ////////////////////// END CLOCK GATING //////////////////////
   
   assign rxf_write_overflow = (rx_cnt_q > 3'h0);


  // RX Counter for OVERFLOW
  always @ (posedge rx_clk)
     if (rxRst) begin 
       rx_overflow_det_q  <= `tCQ 1'h0;
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
    else if (rxClkEn) begin
       if (rx_overflow_det)
	 rx_overflow_det_q  <= `tCQ 1'h1;
       else if (rx_cnt_q == 3'h5)
	 rx_overflow_det_q  <= `tCQ 1'h0;
    end //  else if (rxClkEn)  
   ////////////////////// END CLOCK GATING //////////////////////

// MAV (7May25) 
   assign clkdRst = (reset || tx_break1 || tx_error1);

//JLPL - 8_2_25   
   assign masked_txf_read = txf_read & enable_read;
   
   always @ (posedge clocki)
	begin
     if (clkiRst)
		enable_read <= 1'b0;
	 else
		enable_read <= txClkEn & ~tx_clk & ~tx_clk_div5 & txf_rdReady; //enable txf_read 1 cycle after _q gets latched for a valid transmit FIFO output
	end //end always
//End JLPL - 8_2_25

   // MAV (23May22) Flop txf_rdata_mux_q for encoder // MAV (5May25) 	   	
   always @ (posedge clocki)
     if (clkiRst) begin
	txf_rdata_mux_q <= `tCQ 9'h0;
     end
     else if (txClkEn && ~tx_clk && ~tx_clk_div5 && preMuxed_SCO_se) begin
	      txf_rdata_mux_q <= `tCQ txf_rdata_mux;
     end


   // MAV (23May22) Flop txf_write for encoder // MAV (5May25) 	   	
   always @ (posedge rx_clk)
     if (rxRst) begin 
	rxf_write_q <= `tCQ 1'h0;
     end
     else if (rxClkEn) begin
	if (~rx_clk_early_div5 && rxf_write) begin
	  rxf_write_q <= `tCQ 1'h1;
	end
	else begin
	  rxf_write_q <= `tCQ 1'h0;
	end
     end // else if (rxClkEn)
    
// Need this to send IDLES when TX is  PAUSED
   always @ (posedge clocki)  // MAV (19May25) was negedge
     begin
	if (clkiRst) begin 
          pause_mask <= `tCQ 1'h0;
	end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (txClkEn && ~tx_clk  && ~tx_clk_div5 && preMuxed_SCO_se) begin      
          if      (txf_rdata_mux == {1'h1, PAUSEACK}) 
	    pause_mask  <= `tCQ 1'b1;  // MAV (21May25) added preMuxed_SCO_se
	  else if (txf_rdata_mux == {1'h1, RESUMEACK})
	    pause_mask  <= `tCQ 1'b0;  // MAV (21May25) added preMuxed_SCO_se
	end
	////////////////////// END CLOCK GATING //////////////////////
     end // always @ (posedge clocki)   

   
   // MAV (15Jun25) Need to delay accepting Break for until after after TXF read   

   // Set when TXF Reac data = BREAK/ERROR written & clear when read 
   always @ (posedge clocki)  // MAV (19May25) was negedge
     begin
	if (clkiRst) begin 
         txf_read_break_error_ext  <= `tCQ 1'h0;
	end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (txClkEn && ~tx_clk  && ~tx_clk_div5 && preMuxed_SCO_se) begin      
	   if ((txf_rdata == {1'h1, BREAK} || txf_rdata == {1'h1, ERROR}) && (txf_rdWrdCnt == 1'b1))     
	    txf_read_break_error_ext  <= `tCQ 1'b1;
	  else if (txf_read_break_error_ext && txf_rdWrdCnt == 1'b0)
	    txf_read_break_error_ext  <= `tCQ 1'b0; 
	end
	////////////////////// END CLOCK GATING //////////////////////
     end // always @ (posedge clocki)   


   // Synchronize from clocki back to clockd domain 
   A2_sync txf_read_break_error_ext_sync_clkd  (.q(txf_read_break_error_ext_sync), .d(txf_read_break_error_ext), .reset(reset) , .clock(clockd));  // MAV (16Jun25)

   // Flop (mask) BREAK & ERROR Write 
    always @ (posedge clockd) 
     if (reset) begin	
	break_error_write_mask <= `tCQ 1'h0;      
     end
     else if (tx_break1)  begin 
	break_error_write_mask <= `tCQ 1'h1;      
     end     
     else if (txf_read_break_error_ext_sync_fall)  begin 
	break_error_write_mask <= `tCQ 1'h0;      
     end   
  
   // FLOP BREAK & ERROR RESET
    always @ (posedge clockd) 
     if (reset) begin	
	txf_read_break_error_ext_sync_q	 <= `tCQ 1'h0;      
     end
     else   begin 
	txf_read_break_error_ext_sync_q	 <= `tCQ txf_read_break_error_ext_sync;      
     end     
  
   // FLOP BREAK & ERROR RESET 
   always @ (posedge clockd) 
     if (reset) begin	
	break_error_reset_q <= `tCQ 1'h0;      
     end
     else begin 
	break_error_reset_q <= `tCQ (tx_break1 || tx_error1);  //(tx_cnt_q == 3'h0 &&        
     end  

   assign txf_read_break_error_ext_sync_fall = (~txf_read_break_error_ext_sync && txf_read_break_error_ext_sync_q);
   

endmodule


