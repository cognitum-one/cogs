//===========================================================
// Karillon Corporation 2021
// Colorado Springs, Colorado, USA
//===========================================================
// Module      : lvds_control.v  
// Description : Control Block 
// Author      : Mike Vigil
// Date        : Feb 3, 2022 
//===========================================================

`ifdef  synthesis //JLSW
   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh"
`else
   `include "A2_project_settings.vh"
`endif

//`timescale 1ps/1fs   // MAV (26May22) -added per Roger's suggestion //JLSW


module lvds_control #(
   parameter   WIDTH    =  32,
   parameter   LG2DEPTH =   5, // DEPTH always a power of 2  // MAV (12Feb25) increased to 5
   parameter   WR_ALMOST_FULL_MAX_OFFSET  =   10,   // MAV (12Feb25) increased to 10   
   parameter   WR_ALMOST_FULL_MIN_OFFSET  =   14,   // MAV (12Feb25) added   
   parameter   WR_ALMOST_EMPTY_OFFSET     =   10    // MAV (20Feb25) increased to 10   
   ) (		      
  // INPUTS
   input wire 		   tx_clk,// System Clock  
   input wire 		   rx_clk,// RX Clock
   input wire 		   clocki,// MAV (17Apr25)
   input wire 		   clockd,// DataSystem clock
//   input wire 		   rx_clk_n,// Inverted RX Clock  // MAV (21Feb22)   
   input wire 		   rx_clk_div5,// System Clock Div5   // MAV (12May) was sco_tx_div5  
//   input wire 		   tx_clk_buf , // TX Serial Clock (single-ended) // MAV (21Feb22)
//   input wire 		   tx_clk_buf_n , // Inverted TX Serial Clock (single-ended) // MAV (21Feb22)
   input wire 		   tx_clk_div5, // Divided TX Parallel Clock // MAV (21Feb22)          		       
   input wire 		   txClkEn, // MAV (30Mar25) added txClkEn port
   input wire 		   rxClkEn, // MAV (31Mar25) added txClkEn port
   input wire 		   clockdEn, // MAV (31Mar25) added clockdEn port
   input wire 		   reset, // Active High Reset
   input wire [8:0] 	   rxf_wdata, // [8] = k_char, [7:0] = data
   input wire [8:0] 	   rxf_rdata, // [8] = k_char, [7:0] = data
   input wire 		   txf_written_once, // MAV (25Apr22) added
   input wire 		   rxf_written_once, // MAV (25Apr22) added
   input wire 		   txf_read_once, // MAV (20Jun22) added
   input wire 		   rxf_read_once, // MAV (20Jun22) added
   input wire [LG2DEPTH:0] txf_wrWrdCnt, // MAV (20Apr22) Replaced previous input flags
   input wire 		   txf_wrReady, // MAV (20Apr22) Replaced previous input  flags
   input wire [LG2DEPTH:0] txf_rdWrdCnt, // MAV (20Apr22) Replaced previous input flags
   input wire 		   txf_rdReady, // MAV (20Apr22) Replaced previous input  flags 
   input wire [LG2DEPTH:0] rxf_wrWrdCnt, // MAV (20Apr22) Replaced previous input flags
   input wire 		   rxf_wrReady, // MAV (20Apr22) Replaced previous input  flags
   input wire [LG2DEPTH:0] rxf_rdWrdCnt, // MAV (20Apr22) Replaced previous input flags
   input wire 		   rxf_rdReady, // MAV (20Apr22) Replaced previous input  flags		      
   input wire 		   rx_idle_det,
   input wire 		   read, // MAV (9May22) // MAV (12May) changed to read
   //OUTPUTS
   output wire 		   rx_error_det, // MAV (9May22) changed to wire                                        // MAV (4May25) was rxf_error
   output wire 		   rx_break_det, // Receive Control code for Break // MAV (9May22) changed to wire      // MAV (4May25) was rxf_break
   output wire 		   rx_overflow_det, // MAV (5May25) RX FIFO Overflow moved from lvds_dfe to lvds_control  
// output wire 		   rxf_sof, // Transmit Control code for Start-of-Frame // MAV (9May22) changed to wire // MAV (4May25) no longer used 
// output wire 		   rxf_eof, // Transmit Control code for End-of-Frame // MAV (9May22) changed to wire   // MAV (4May25) no longer used
   output wire [8:0] 	   rxf_wdata_out, // MAV (4May25) RX FIFO write Data Muxed 
   output wire 		   rxf_pause, // Transmit Control code for Pause   // MAV (29Jan25) now a wire
   output wire 		   rxf_resume, // Transmit Control code for Resume // MAV (29Jan25) now a wire
   output wire 		   capture, // Receive PISO Parallel Capture
   output reg 		   load, // Transmit SIPO Parallel Load
   output reg 		   txf_read, // Transmit FIFO read  
   output reg 		   rxf_write, // Receive FIFO write 
   output wire 		   rxf_read, // Receive FIFO read  // MAV (12May22)
   output wire 		   txf_resume_ack, //  MAV (23May22) RESUME acknowledged (was txf_rdAgain)
   output wire 		   txf_pause_ack, //  MAV (13Feb25) PAUSE acknowledged
   input wire 		   fifo_almost_full_select, //  MAV (13Feb25) Almost Full Select 
   /// MAV (6Apr25) 
   input wire 		   clkiRst,// MAV (17Apr25) RESET synced to clocki
// input wire 		   txRst, // MAV (6Apr24) RESET synced to TX Clk (clocki_div2_quad)  // MAV (15Apr25) changed to input (18Apr25) replaced with clkiRst
   input wire 		   rxRst, // MAV (6Apr24) RESET synced to TX Clk (SCI_se)           // MAV (15Apr25) changed to input
   output wire [7:0] 	   ctlStatus, //  MAV (6May25)
   input wire              preMuxed_SCO_se);   // MAV (19May25) added
   //    
  ////////////////////////// Parameters //////////////////////////
   localparam       SOF    = 8'hFB;
   localparam       EOF    = 8'hFD;
   localparam       BREAK  = 8'hF7;
   localparam       ERROR  = 8'hFE;
   localparam       PAUSE  = 8'h1C;
   localparam       RESUME = 8'h3C;
   localparam       IDLE   = 8'hBC;
   localparam       RESUMEACK = 8'h5C;  // MAV (23May22) 
   localparam       PAUSEACK  = 8'h7C;  // MAV (13Feb25)
   localparam       OVERFLOW   = 8'hFC; // MAV (23May25)
   localparam       RESERVED9C = 8'h9C; // MAV (23May25)   
   localparam       RESERVEDDC = 8'hDC; // MAV (23May25)
   
   ////////////////////////// Regs & Wires //////////////////////////
   reg [2:0] 	    cap_cntr;  // Capture Counter 
   reg  rxf_write_init; // MAV (12May22)
// reg 	rxf_sof_q, rxf_eof_q, rxf_break_q, rxf_error_q;  // MAV (22Jun22) // (4May25) no longer used 
   reg 	rxf_packet_q;    // MAV (19Mar25) //  MAV (31Mar25)  moved to here 
   reg 	tx_clk_div10_read_ok;  // MAV (23Apr25) DIV5 Mask for TX FIFO Read
   reg [2:0]	       rx_cnt_q; // Counters for break & reset // added (4May25)
   // MAV - need to build flags with Counter and/or Ready Inputs 
   assign rxf_wr_full_flag       = (rxf_wrWrdCnt == ((2 ** LG2DEPTH)-1)); // MAV (25Apr22)
   // MAV (22Apr22)
//   wire rxf_wr_almost_full_flag  = (rxf_written_once && (rxf_wrWrdCnt >= ((2 ** LG2DEPTH) - WR_ALMOST_FULL_OFFSET)));
   wire rxf_wr_almost_empty_flag = (rxf_written_once && (rxf_wrWrdCnt <= WR_ALMOST_EMPTY_OFFSET));
   // MAV (12May22) internal only
   wire rx_sof_det      = (rxf_wdata == {1'h1, SOF}); 
   wire rx_eof_det      = (rxf_wdata == {1'h1, EOF});
   wire rx_pause_det    = (rxf_wdata == {1'h1, PAUSE}); // MAV (31Mar25) added && rxf_packet_q)    // (1Apr25) removd  && rxf_packet_q
   wire rx_resume_det   = (rxf_wdata == {1'h1, RESUME}); // MAV (31Mar25) added && rxf_packet_q)   // (1Apr25) removd  && rxf_packet_q   
   wire rxf_write_early = (~rxf_wr_full_flag && rx_sof_det); // MAV (12May22) internal only
   wire rxf_resumeAck_det = (rxf_wdata == {1'h1, RESUMEACK});   // MAV (28Jan25) added _det  // (13Feb25) was rxf_wrAgain_det & WRAGAIN // MAV (31Mar25) added && rxf_packet_q)    // (1Apr25) removd  && rxf_packet_q   
   wire rx_idle_det2    = (rxf_wdata == {1'h1, IDLE});  // MAV (23May22)
   assign rx_error_det  = ((rxf_wdata == {1'h1, ERROR}) && (rx_cnt_q == 3'h0)); // MAV (27May22)        // (4May25) changed wire to assign (4May25) added && (rx_cnt_q == 3'h0)
   assign rx_break_det  = ((rxf_wdata == {1'h1, BREAK}) && (rx_cnt_q == 3'h0)); // MAV (16Feb25) added  // (4May25) changed wire to assign (4May25) added && (rx_cnt_q == 3'h0)
   wire rxf_pauseAck_det = (rxf_wdata == {1'h1, PAUSEACK});   // MAV (13Feb25) added  // MAV (31Mar25) added && rxf_packet_q)  // (1Apr25) removd  && rxf_packet_q
   wire pauseOverlapPauseAck, pauseOverlapResumeAck, resumeOverlapPauseAck, resumeOverlapResumeAck;	// MAV (10Mar25) added
   
   //  MAV (1May25) New logic
   wire 	       rx_break2, rx_error2; // RX BREAK & ERROR
   reg 		       rx_break_ext;         // RX extended BREAK = 1 ERROR = 0

   // MAV (13Feb25) programmable almost_full_offset
   wire [LG2DEPTH:0] wr_almost_full_offset    = (fifo_almost_full_select)? WR_ALMOST_FULL_MIN_OFFSET :  WR_ALMOST_FULL_MAX_OFFSET;
   wire              rxf_wr_almost_full_flag  = (rxf_written_once && (rxf_wrWrdCnt >= ((2 ** LG2DEPTH) - wr_almost_full_offset)));
	   
   reg 	     txf_read2, txf_read_mask2;  // MAV (23Feb25)  actually wires (18May25) removed txf_read1, 
   wire      rxf_write_ok, rxf_write_comma_char;    // MAV (19Mar25) // MAV (23May25) added rxf_write_comma_char

   // Capture counter
   always @ (posedge rx_clk)   // MAV (6Apr25) removed posedge reset or 
     begin
        if (rxRst)            // MAV (6Apr25) was (reset)
	  begin
             cap_cntr  <= `tCQ 3'h0;      
	  end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
     else if (rxClkEn) begin // MAV (31Mar25) added if (rxClkEn)
	if (rx_idle_det)     // MAV (31Mar25) removed else
	  begin
	     cap_cntr  <= `tCQ 3'h0;      
       	  end
	else if (cap_cntr >= 4)
	  begin
             cap_cntr  <= `tCQ 3'h0;      
       	  end
	else 
	  begin	
	     cap_cntr  <= `tCQ  cap_cntr + 4'h1; 	     
	  end   
     end   
   ////////////////////// END CLOCK GATING //////////////////////
    end   //  else if (rxClkEn)
   
   assign capture = (cap_cntr == 3'h0);  // MAV (15Feb25) was 0


   // Generate Load pulse
   always @ (posedge clocki)  // MAV (6Apr25) removed posedge reset or (16Apr25) was posedge tx_clk (19May25) was negedge
     begin
	if (clkiRst) begin  // MAV (6Apr25) was (reset) (16Apr25) was txRst
          load  <= `tCQ 1'h0;
	end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (txClkEn  && ~tx_clk && preMuxed_SCO_se) begin // MAV (30Mar25) added if (txClkEn) (16Apr25) added && ~tx_clk (19May25) added preMuxed_SCO_se      
          load  <= `tCQ ~tx_clk_div5;   // MAV (18May22) was rx_clk_div5
	end
	////////////////////// END CLOCK GATING //////////////////////
     end // always @ (posedge reset or posedge tx_sclk
   
   
   // RX FIFO Write     
   always @ (posedge rx_clk)   // MAV (6Apr25) removed posedge reset or 
     if (rxRst) begin         // MAV (6Apr25) was (reset)
	rxf_write_init <= `tCQ 1'h0;
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
     else if (rxClkEn && ~rx_clk_div5) begin     // MAV (31Mar25) added if (rxClkEn)  // MAV (23May25) moved ~rx_clk_div5 here
	if (~rxf_wr_full_flag &&                 // MAV (31Mar25) removed else        // MAV (23May25) moved ~rx_clk_div5 to above 
	      (rx_sof_det || rxf_resume)) begin  // MAV (12May22)- changed to rx_sof_det // (16May22) added || rxf_resume
	   rxf_write_init <= `tCQ 'h1; 
     end
     else if (rx_eof_det || rx_error_det || rx_break_det)  begin // MAV (23May25) moved  ~&& rx_clk_div5 to above 
	rxf_write_init <= `tCQ 'h0;  // MAV (27May22) somehow deleted and added back in 
    end	   
   ////////////////////// END CLOCK GATING //////////////////////
    end   //  else if (rxClkEn)
   
// MAV (9May22)  <-- moved into CONTROL & simplified with rx_XXX_det
   wire rxf_write_temp  = ((rxf_write_init || rxf_write_early || rx_break2  || rx_error2) && ~rx_idle_det2 && // MAV (4May25) added rx_break2  || error2
			   ~rx_pause_det && ~rx_resume_det && ~rxf_resumeAck_det && rxf_wrReady && // MAV (28Jan25) added _det  // (13Feb25) was rxf_wrAgain_det
			   ~rxf_pauseAck_det && ~rx_break_det && ~rx_error_det);   // MAV (14Feb25) added && ~rxf_pauseAck_det removed ~rxf_write_mask &&  (4May25) added ~rx_*_det

   always @ (*)  rxf_write = ((rxf_write_temp && rxf_write_ok) ||  // MAV (1Nov24) fix race condition // MAV (19Mar25) added rxf_write_ok
			      rx_break2 || rx_error2 || rxf_write_comma_char); // (4May25) added rx_break2 || rx_error2 // (23May25) added rxf_write_comma_char
   
   assign rxf_read = (read && rxf_rdReady);  // MAV (22May22) was  && ~rxf_read_mask);  


//////////////////////////////////////////////////////////////////////////
// MAV (19Feb25)   
   reg  rxf_pause_ext2_q,         rxf_pause_ack_ext2_q;
   wire rxf_pause_ext2_q_sync,    rxf_pause_ack_det_q_sync,    bogus_rxf_pauseAck_q_sync;    // MAV (10Mar25) added bogus_rxf_pauseAck_q_sync
   reg  rxf_pause_ext2_q_sync_q,  rxf_pause_ack_det_q_sync_q,  bogus_rxf_pauseAck_q_sync_q;  // MAV (10Mar25) added bogus_rxf_pauseAck_q_sync_q
   reg  rxf_pause_ext2_q_sync_q1, rxf_pause_ack_det_q_sync_q1, bogus_rxf_pauseAck_q_sync_q1; // MAV (10Mar25) added bogus_rxf_pauseAck_q_sync_q1
   reg  bogus_rxf_pauseAck_q;    // MAV (3Mar25),     bogus_rxf_pauseAck_q1, bogus_rxf_pauseAck_q2; // MAV (29Mar25) removed piped flops
   
// NAV (10Mar25) seperated from below flop code
   always @ (posedge rx_clk)  // MAV (6Apr25) removed posedge reset or // (18Apr25) was rx_clk_div5
     if (rxRst) begin             // MAV (6Apr25) was (reset)
	bogus_rxf_pauseAck_q  <= `tCQ 1'h0;    
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
     else if (rxClkEn && ~rx_clk_div5) begin // MAV (31Mar25) added if (rxClkEn) // (18Apr25) added && ~rx_clk_div5
      if (~rxf_pause_ext2_q && rxf_pauseAck_det) begin  // MAV (31Mar25) removed else
	bogus_rxf_pauseAck_q  <= `tCQ 1'h1; 
     end 
     else if (rxf_resumeAck_det) begin
	bogus_rxf_pauseAck_q  <= `tCQ 1'h0;
     end
   ////////////////////// END CLOCK GATING //////////////////////
   end   //  else if (rxClkEn)


       
   always @ (posedge rx_clk)  // MAV (6Apr25) removed posedge reset or // (18Apr25) was rx_clk_div5
     if (rxRst) begin         // MAV (6Apr25) was (reset)
	rxf_pause_ext2_q  <= `tCQ 1'h0; 
     end
//     else if (rxf_wr_almost_full_flag || (~rxf_pause_ext2_q && rxf_pauseAck_det)) begin // MAV (1Mar25) added || (~rxf_ ..._det)  // 3Mar25 REMOVED || (~rxf_... _det)
   ////////////////////// BEGIN CLOCK GATING //////////////////////
     else if (rxClkEn && ~rx_clk_div5) begin // MAV (31Mar25) added if (rxClkEn) // (18Apr25) added && ~rx_clk_div5
       if (rxf_wr_almost_full_flag) begin // MAV (10Mar25) reverted to previous code // MAV (31Mar25) removed else
	rxf_pause_ext2_q  <= `tCQ 1'h1; 
     end
     else if ((rxf_wr_almost_empty_flag && rxf_pause_ext2_q) ||        // MAV (10Mar25) reverted to previous code 
              (~rxf_pause_ext2_q && rxf_pauseAck_det )       ||        // MAV (10Mar25) reverted to previous code 
	      (rx_error_det || rx_break_det || rx_eof_det)) begin 
	rxf_pause_ext2_q  <= `tCQ 1'h0; 
     end
   ////////////////////// END CLOCK GATING //////////////////////
    end   //  else if (rxClkEn)
	
  always @ (posedge rx_clk)  // MAV (6Apr25) removed posedge reset or // (18Apr25) was rx_clk_div5
    if (rxRst) begin             // MAV (6Apr25) was (reset)
	rxf_pause_ack_ext2_q  <= `tCQ 1'h0;
     end
  ////////////////////// BEGIN CLOCK GATING //////////////////////
     else if (rxClkEn && ~rx_clk_div5) begin // MAV (31Mar25) added if (rxClkEn) // (18Apr25) added && ~rx_clk_div5
       if (rx_pause_det) begin  // MAV (31Mar25) removed else
	rxf_pause_ack_ext2_q  <= `tCQ 1'h1; 
     end
     else if (rx_resume_det || rx_error_det || rx_break_det) begin 
	rxf_pause_ack_ext2_q  <= `tCQ 1'h0; 
     end
   ////////////////////// END CLOCK GATING //////////////////////
    end   //  else if (rxClkEn)

   
   // Synchronize from RX to TX clock domain - sources are now multi-cycle
   A2_sync pause_ext_sync2  (.q(rxf_pause_ext2_q_sync),    .d(rxf_pause_ext2_q),     .reset(clkiRst), .clock(clocki));  // MAV (25Fev25) was .resetn(~reset) (18Apr25) replaced txRst with clkiRst // tx_clk
   A2_sync pause_det_sync2  (.q(rxf_pause_ack_det_q_sync), .d(rxf_pause_ack_ext2_q), .reset(clkiRst), .clock(clocki));  // MAV (25Fev25) was .resetn(~reset) (18Apr25) replaced txRst with clkiRst // tx_clk
   A2_sync bogus_pause_sync2(.q(bogus_rxf_pauseAck_q_sync),.d(bogus_rxf_pauseAck_q), .reset(clkiRst), .clock(clocki));  // MAV (25Fev25) was .resetn(~reset) (18Apr25) replaced txRst with clkiRst // tx_clk
   

   always @ (posedge clocki)   // MAV (6Apr25) removed posedge reset or (18Apr25) was posedge tx_clk_div5 (19May25) was negedge
     if (clkiRst) begin        // MAV (6Apr25) was (reset) (18Apr25) replaced txRst with clkiRst
	rxf_pause_ext2_q_sync_q        <= `tCQ 1'h0;
	rxf_pause_ext2_q_sync_q1       <= `tCQ 1'h0;
	rxf_pause_ack_det_q_sync_q     <= `tCQ 1'h0;
	rxf_pause_ack_det_q_sync_q1    <= `tCQ 1'h0;
	bogus_rxf_pauseAck_q_sync_q    <= `tCQ 1'h0; // MAV (10Mar25) added
	bogus_rxf_pauseAck_q_sync_q1   <= `tCQ 1'h0; // MAV (10Mar25) added input_ready

     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
     else if (txClkEn && ~tx_clk && ~tx_clk_div5 && preMuxed_SCO_se) begin  // MAV (30Mar25) added if (txClkEn) (18Apr25) added && ~tx_clk && ~tx_clk_div5 (19May25) added preMuxed_SCO_se
	rxf_pause_ext2_q_sync_q        <= `tCQ rxf_pause_ext2_q_sync;
	rxf_pause_ext2_q_sync_q1       <= `tCQ rxf_pause_ext2_q_sync_q;
	rxf_pause_ack_det_q_sync_q     <= `tCQ rxf_pause_ack_det_q_sync;
	rxf_pause_ack_det_q_sync_q1    <= `tCQ rxf_pause_ack_det_q_sync_q;
	bogus_rxf_pauseAck_q_sync_q    <= `tCQ bogus_rxf_pauseAck_q_sync;   // MAV (10Mar25) added
	bogus_rxf_pauseAck_q_sync_q1   <= `tCQ bogus_rxf_pauseAck_q_sync_q; // MAV (10Mar25) added
     end
   ////////////////////// END CLOCK GATING //////////////////////


   // MAV (20Feb25) New pause/resume
   wire pause2        =  rxf_pause_ext2_q_sync_q      && ~rxf_pause_ext2_q_sync_q1;
   wire resume2       = ~rxf_pause_ext2_q_sync_q      &&  rxf_pause_ext2_q_sync_q1;
   wire pauseAck2     =  rxf_pause_ack_det_q_sync_q   && ~rxf_pause_ack_det_q_sync_q1;
   wire resumeAck2    = ~rxf_pause_ack_det_q_sync_q   &&  rxf_pause_ack_det_q_sync_q1;
   wire bogusResume2  =  bogus_rxf_pauseAck_q_sync_q  && ~bogus_rxf_pauseAck_q_sync_q1; // MAV (10Mar25) added
      
   always @ (*) txf_read_mask2 = rxf_pause_ack_det_q_sync_q || rxf_pause_ack_det_q_sync_q1; 

   always @ (*) txf_read2 = (txf_written_once && txf_rdReady && ~txf_read_mask2 &&  // MAV (11Mar25) moved ~txf_read_mask2 here // removed <
//			      ~pause2 && ~resume2 &&                                  // MAV (11Mar25) replaced as shown below
//			      ~txf_read_mask2 && ~resumeAck2 && ~pauseAck2);          // MAV (11Mar25) replaced as shown below
			      ~rxf_pause && ~rxf_resume && ~txf_resume_ack && ~txf_pause_ack && // MAV (11Mar25) replaced above
                              tx_clk_div10_read_ok);
   
   // MAV (10Mar25) Overlap scenarios flopped signals
   wire overlapPauseAck,   overlapResumeAck;   // overlapBogusResume; 
   reg  overlapPauseAck_q, overlapResumeAck_q; // overlapBogusResume_q; 
   
   
   // MAV (10Mar25) Overlap scenarios
   assign pauseOverlapPauseAck     = pause2  && pauseAck2;
   assign pauseOverlapResumeAck    = pause2  && resumeAck2;
   assign resumeOverlapPauseAck    = resume2 && pauseAck2;
   assign resumeOverlapResumeAck   = resume2 && resumeAck2;
   assign overlapPauseAck          = (pauseOverlapPauseAck  || resumeOverlapPauseAck);
   assign overlapResumeAck         = (pauseOverlapResumeAck || resumeOverlapResumeAck);
// assign overlapBogusResume       = (pauseOverlapPauseAck  || resumeOverlapPauseAck);

   // MAV (10Mar25) Flop to delay if overlapped
   always @ (posedge clocki)   // MAV (6Apr25) removed posedge reset or (18Apr25) was posedge tx_clk_div5 (19May25) was negedge
     if (clkiRst) begin        // MAV (6Apr25) was (reset) (18Apr25) replasced txRst with clkiRst
	overlapPauseAck_q    <= `tCQ 1'h0;
	overlapResumeAck_q   <= `tCQ 1'h0;
//	overlapBogusResume_q <= `tCQ 1'h0;
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
     else if (txClkEn && ~tx_clk && ~tx_clk_div5 && preMuxed_SCO_se) begin  // MAV (30Mar25) added if (txClkEn) (18Apr25) added && ~tx_clk && ~tx_clk_div5 (19May25) added preMuxed_SCO_se
	overlapPauseAck_q    <= `tCQ overlapPauseAck;
	overlapResumeAck_q   <= `tCQ overlapResumeAck;
//	overlapBogusResume_q <= `tCQ overlapBogusResume;
     end
   ////////////////////// END CLOCK GATING //////////////////////

	

  // MAV (20Feb25) New pause/resume muxing 
   assign rxf_pause      = pause2;
   assign rxf_resume     = (resume2 || bogusResume2); 
   assign txf_pause_ack  = (overlapPauseAck_q) ? 1'b1 : (pauseAck2  && ~overlapPauseAck);  
   assign txf_resume_ack = (overlapResumeAck_q)? 1'b1 : (resumeAck2 && ~overlapResumeAck);
   always @ (*) txf_read = txf_read2;


   always @ (posedge rx_clk)   // MAV (6Apr25) removed posedge reset or 
     if (rxRst) begin         // MAV (6Apr25) was (reset)
       rxf_packet_q <= `tCQ 1'h0;
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
     else if (rxClkEn && ~rx_clk_div5) begin   // MAV (31Mar25) added if (rxClkEn)  // MAV (23May25) added ~rx_clk_div5
      if (rx_sof_det) begin    // MAV (31Mar25) removed else
       rxf_packet_q <= `tCQ 'h1; 
     end
     else if (rx_eof_det || rx_error_det || rx_break_det || rx_overflow_det) begin  // added rx_*_det)
       rxf_packet_q <= `tCQ 'h0; 
    end	   
   ////////////////////// END CLOCK GATING //////////////////////
  end   //  else if (rxClkEn)


   assign rxf_write_ok = (rx_sof_det || rxf_packet_q || (rx_eof_det && rxf_packet_q));  // MAV (19Mar25) // MAV (5May25) added  && rxf_packet_q
   		
//////////////////////////////////////////////////////////////
// UPDATED BREAK & ERROR logic 
//////////////////////////////////////////////////////////////

// rx_cnt_q starts counting for either BREAK or ERROR rolls over to 0 then stops counting
   always @ (posedge rx_clk)   // MAV (6May25) was rx_clk_div5
     if (rxRst) begin
	rx_cnt_q <= `tCQ 3'h0;
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
     else if (rxClkEn && ~rx_clk_div5 && (rx_break_det || rx_error_det || rx_cnt_q > 3'h0)) begin // MAV (6May25) added ~rx_clk_div5
 	rx_cnt_q <= `tCQ rx_cnt_q + 3'h1;
   ////////////////////// END CLOCKGATING //////////////////////
  end   //  else if (rxClkEn)
   
// rx_break_ext = 1 for BREAK or rx_break_ext = 0 for ERROR
   always @ (posedge rx_clk)   // MAV (6May25) was rx_clk_div5
     if (rxRst) begin
	rx_break_ext <= `tCQ 1'h0;
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
     else if (rxClkEn && ~rx_clk_div5) begin  // MAV (6May25) added ~rx_clk_div5
	if (rx_break_det && (rx_cnt_q == 3'h0)) begin
	  rx_break_ext <= `tCQ 1'h1;
	end
	else if (rx_cnt_q == 3'h6) begin
	   rx_break_ext <= `tCQ 1'h0;
	end
   ////////////////////// END CLOCKGATING //////////////////////
  end   //  else if (rxClkEn)


// rx_break_ext = 1 for BREAK or rx_break_ext = 0 for ERROR
   assign rx_break2 = ( rx_break_ext && (rx_cnt_q == 3'h5));
   assign rx_error2 = (~rx_break_ext && (rx_cnt_q == 3'h5));

// mux data to RX FIFO
   assign rxf_wdata_out = (rx_break2)? {1'h1, BREAK} : 
			  (rx_error2)? {1'h1, ERROR} :
			  rxf_wdata; 

   assign rx_overflow_det  = (rxf_write && rxf_wr_full_flag);  // MAV (5May25) 

   // MAV (6May25) added control signals for visabilty
   assign ctlStatus = {3'b0, rxf_pause_ext2_q, rxf_pause_ack_ext2_q, bogus_rxf_pauseAck_q, rxf_packet_q, rx_break_ext};

// MAV (23Apr25) DIV10 Mask for TX FIFO Read 	
   always @ (posedge clocki)  // MAV (19May25) was negedge
     begin
	if (clkiRst) begin 
         tx_clk_div10_read_ok  <= `tCQ 1'h0;
	end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (txClkEn) begin      
          tx_clk_div10_read_ok  <= `tCQ (~tx_clk  && ~tx_clk_div5 /*&& ~tx_clk_div10_read_ok*/ && preMuxed_SCO_se);  // MAV (21May25) added preMuxed_SCO_se
	end
	////////////////////// END CLOCK GATING //////////////////////
     end // always @ (posedge clocki)
 
   // MAV (23May25) - added to allow RXF FIFO writes for Reserved Comma Characters
   assign rxf_write_comma_char = ((rxf_wdata == {1'h1, RESERVED9C}) ||
				  (rxf_wdata == {1'h1, RESERVEDDC}));
  

endmodule


