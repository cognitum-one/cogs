//===========================================================
// Karillon Corporation 2021
// Colorado Springs, Colorado, USA
//===========================================================
// Module      : lvds_afe.v  Buffered version of  SCI_se
// Description : Digital Clock Module
// Author      : Mike Vigil
// Date        : May 17, 2022 
//===========================================================

`ifdef  synthesis //JLSW
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2_project_settings.vh"
`else
   `include "A2_project_settings.vh"
`endif

//`timescale 1ps/1fs   // MAV (26May22) -added per Roger's suggestion //JLSW
module lvds_clock
  // INPUTS
  (
   input wire  reset, // reset IN from LVDS core
   input wire  clockd, // System Clock   // MAV (6Apr25) 
   input wire  clocki, // System TX Clock 
   input wire  SCI_se, // Same as rx_clk but best to unbuffered 
   input wire  SCI_se_n, // Inverted  SCI_se_n
   input wire  txClkEn, // MAV (30Mar25) added txClkEn port
   input wire  rxClkEn, // MAV (31Mar25) added rxClkEn port
   input wire  rx_idle_det, // MAV (2Apr24) used to re-sync rx_clk_div5 
   // OUTPUTS
   output wire rx_clk, // Buffered version of SCI_se
   output wire rx_clk_n, // Buffered & inverted version of SCI_se
   output wire rx_clk_div5, // Divided Parallel Clock Out to LVDS core 
   output wire rx_clk_early_div5, // Early Divided Parallel Clock Out to LVDS core    // MAV (23May25) added
   output wire tx_clk, // Buffered TX Clock // MAV (7Apr25) removed _buf
   output wire tx_clk_n, // Buffered & inverted System Clock // MAV (7Apr25) removed _buf
   output wire tx_clk_div5, // Divi   output wire clkiRst, // MAV (17Apr25) RESET synced to clocki
   /// MAV (6Apr25) 
   output wire clkiRst,// MAV (6Apr24) RESET synced to clocki  
// output wire txRst, // MAV (6Apr24 ) RESET synced to TxClk  (18Apr25) replaced with clkiRst
   output wire rxRst, // MAV (6Apr24) RESET synced to RxClk
   output wire clocko, // MAV (7Apr25) moved from lvds_dfe
   output wire preMuxed_SCO_se); // MAV (7Apr25) moved from lvds_dfe
   
  
  ////////////////////////// Regs & Wires //////////////////////////
// reg [2:0]   tx_clk_cntr, rx_clk_cntr;  // Counters for Div/5 clocks
   reg [4:0]   tx_clk_cntr_1hot, rx_clk_cntr_1hot; // MAV (23May25) One-hot counters 
   reg 	       clocki_div2; // MAV (6Apr25) moved from lvds_dfe // (19May25) removed clocki_div2
   reg 	       clocki_div4, clocki_div4_quad;   // (19May25) added

   // MAV (23May25) replaced TX & RX Div5 counters with one-hot counters at the end of the code

   // Assign Clocks  // MAV (26May22) - removed #1
   assign rx_clk       =  SCI_se;  // BUFFER
   assign rx_clk_n     =  SCI_se_n;  // MAV (13Dec24) replaced INVERTER with BUFFER 
   assign rx_clk_div5  = (rx_clk_cntr_1hot != 3'h4) ? 1'b1: 1'b0;    // MAV (23May25) changed to one-hot & was <= 3
   assign tx_clk       =  clocki_div4_quad;    // MAV (21Feb22) // MAV (9Apr25) was clocki // MAV (7Apr25) removed _buf
   assign tx_clk_n     = ~clocki_div4_quad;    // MAV (21Feb22) // MAV (9Apr25) was clocki // MAV (7Apr25) removed _buf
   assign tx_clk_div5  = (tx_clk_cntr_1hot != 3'h4) ? 1'b1: 1'b0;    // MAV (23May25) changed to one-hot was <= 3
`ifdef synthesis
   BUFH_X1N_A7P5PP84TR_C14 i_lvds_clk (.Y(clocko), .A(clockd));
`else
   assign clocko       =  clockd;             // MAV (7Apr25) moved from lvds_dfe
`endif
   assign preMuxed_SCO_se   =  clocki_div4;   // MAV (7Apr25) replaced clocki_div2   (19May25) was clocki_div2  
   assign preMuxed_SCO_se_n = ~clocki_div4;   // MAV (7Apr25) replaced clocki_div2_n (19May25) was ~clocki_div2

   // MAV (3Feb25) clocki_div2 must always proceed clocki_div2_quad for proper alignment
   always @(posedge clocki)   // MAV (6Apr25) removed posedge reset or 
     if (clkiRst) begin       // MAV (6Apr25) was (reset)
	clocki_div2 <= `tCQ 1'h0;
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
    else if (txClkEn) begin   // MAV (30Mar25) added if (txClkEn) 
	clocki_div2 <= `tCQ ~clocki_div2;
     end
   ////////////////////// END CLOCK GATING //////////////////////

   // MAV (29Oct24) clocki_div2_quad (19May25) no longer used

   

   // MAV (6Apr25) Reset synchronizers 
// A2_sync_Rst1 clkiRst_sync (.q(clkiRst),     .d(reset), .reset(reset), .clock(clocki)); // MAV (14Apr25) replace with A2_reset //***
// A2_sync_Rst1 txRst_sync   (.q(txRst),       .d(reset), .reset(reset), .clock(clocki)); // MAV (14Apr25) replace with A2_reset //*** // was tx_clk  (18Apr25) replaced with clkiRst
// A2_sync_Rst1 rxRst_sync   (.q(rxRst),       .d(reset), .reset(reset), .clock(SCI_se)); // MAV (14Apr25) replace with A2_reset //***
   A2_reset clkiRst_sync     (.q(clkiRst),                .reset(reset), .clock(clocki)); // MAV (14Apr25) replaced with A2_sync_Rst1 //***
// A2_reset txRst_sync       (.q(txRst),                  .reset(reset), .clock(tx_clk)); // MAV (14Apr25) replaced with A2_sync_Rst1 //*** (18Apr25) replaced with clkiRst
   A2_reset rxRst_sync       (.q(rxRst),                  .reset(reset), .clock(SCI_se)); // MAV (14Apr25) replaced with A2_sync_Rst1 //***




   // MAV (19May25) switched to div4 to ensure rising edge clocki used 
   always @(posedge clocki)   // MAV (6Apr25) removed posedge reset or 
     if (clkiRst) begin       // MAV (6Apr25) was (reset)
	clocki_div4 <= `tCQ 1'h0;
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
    else if (txClkEn && ~clocki_div2) begin   // MAV (30Mar25) added if (txClkEn) 
	clocki_div4 <= `tCQ ~clocki_div4;
     end
   ////////////////////// END CLOCK GATING //////////////////////

   
   // MAV (19May25) clocki_div4_quad 
   always @(posedge clocki)
     if (clkiRst) begin 
	clocki_div4_quad   <= `tCQ 1'h0;
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
    else if (txClkEn && clocki_div2)  begin 
	clocki_div4_quad   <= `tCQ ~clocki_div4_quad;
    end
   ////////////////////// END CLOCK GATING //////////////////////



//////////////////////////////// ONE HOT COUNTERS ////////////////////////////////
    // TX Div5 counter
   always @ (posedge clocki)  // MAV (6Apr25) removed posedge reset & _buf then changed to ~clocki (19May25) removed ~
     if (clkiRst) begin          // MAV (6Apr25) was (reset) then txRst
	tx_clk_cntr_1hot <= `tCQ 5'b00001;     
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
     else if (txClkEn && clocki_div4 && ~clocki_div4_quad)  begin 
	tx_clk_cntr_1hot <= {tx_clk_cntr_1hot[3:0], tx_clk_cntr_1hot[4]}; 
     end // if (txClkEn)   ////////////////////// END CLOCK GATING //////////////////////


   //RX Div5 counter
   always @ (posedge rx_clk)
     if (rxRst) begin 
	rx_clk_cntr_1hot <= `tCQ 5'b00001;
     end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
     else if (rxClkEn) begin 
	if (rx_idle_det) begin 
 	   rx_clk_cntr_1hot <= 5'b01000;  
	end 
	else  begin
	   rx_clk_cntr_1hot <= {rx_clk_cntr_1hot[3:0], rx_clk_cntr_1hot[4]}; 
	end
     end   //  else if (rxClkEn)
   ////////////////////// END CLOCK GATING //////////////////////

   // MAV (24May25) For earlier write_q 
   assign rx_clk_early_div5  = (rx_clk_cntr_1hot != 3'h2) ? 1'b1: 1'b0;    // MAV (23May25)    
   
   
endmodule
  
