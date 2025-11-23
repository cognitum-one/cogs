//===========================================================
// Karillon Corporation 2021
// Colorado Springs, Colorado, USA
//===========================================================
// Module      : lvds_piso.v  
// Description : Parallel to Serial (Serializer) 
// Author      : Mike Vigil
// Date        : Feb 3, 2022 
//===========================================================

`ifdef  synthesis //JLSW
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2_project_settings.vh"
`else
   `include "A2_project_settings.vh"
`endif

//`timescale 1ps/1fs   // MAV (26May22) -added per Roger's suggestion //JLSW


module lvds_piso
  (
   input wire 	     clocki, // MAV (17Apr25) added
   input wire 	     tx_sclk, // high speed quadrature data clock
   input wire 	     tx_sclk_n, // inverted high speed quadrature data clock
   input wire 	     txClkEn, // MAV (30Mar25) added txClkEn port
   input wire 	     reset, // active high reset
   input wire 	     load, // active high load strobe
   input wire [ 9:0] pdata, // 8b10b parallel data
   input wire 	     preMuxed_SCO_se, // MAV (19May) high speed serial clock
   output wire 	     sdata); // serialized data


   ////////////////////////// Regs & Wires //////////////////////////
   reg [ 4:0] 	     sdata_pos_q, sdata_neg_q;
   reg  	     posclk_rise_q, negclk_rise_q;
   reg  	     sdata_pos_zero_q, sdata_neg_zero_q;	     
   reg 		     posclk_rise_det_q;  // MAV (1Jun22) (21Apr25) added _q 	     

   /////////////////////////////////////////////////////////////////////////////
   // Create Clock Positive Edge Rising Pulse
   always @ (posedge clocki)  // MAV (6Apr25) removed posedge reset or (16Apr25) was posedge tx_sclk  (19May25) was negedge
     begin
	if (reset) 
	  posclk_rise_q  <= `tCQ 'h0;
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (txClkEn && ~tx_sclk && posclk_rise_det_q && preMuxed_SCO_se) begin   // MAV (30Mar25) added if (txClkEn) (16Apr25) added && ~tx_sclk (21Apr25) added ~posclk_rise_det_q (19May25) added preMuxed_SCO_se
 	  posclk_rise_q  <= `tCQ ~posclk_rise_q;
	end
   ////////////////////// END CLOCK GATING //////////////////////
     end // always @ (posedge reset or posedge tx_sclk_n)


   // MAV (1Jun22) need in order to align posclk_rise_q followed by negclk_rise_q
   always @ (posedge clocki)  // MAV (6Apr25) removed posedge reset or (16Apr25) was posedge tx_sclk  (19May25) was negedge
     begin
	if (reset) begin
	  posclk_rise_det_q   <= `tCQ 'h0;  // MAV (21Apr25) added _q 	     
	end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (txClkEn && ~tx_sclk && preMuxed_SCO_se) begin   // MAV (30Mar25) added if (txClkEn) (16Apr25) added && ~tx_sclk (19May25) added preMuxed_SCO_se
 	  posclk_rise_det_q  <= `tCQ 'h1;                // MAV (21Apr25) added _q 	
	end
   ////////////////////// END CLOCK GATING //////////////////////
     end // always @ (posedge reset or posedge tx_sclk_n)

  
   // Create Clock Negative Edge Rising Pulse
   always @ (posedge clocki)  // MAV (6Apr25) removed posedge reset or (16Apr25) was posedge tx_sclk  (19May25) was negedge
     begin
	if (reset) 
	  negclk_rise_q  <= `tCQ 'h0;
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (txClkEn && tx_sclk && ~preMuxed_SCO_se) begin   // MAV (30Mar25) added if (txClkEn) (16Apr25) added && tx_sclk (19May25) added preMuxed_SCO_se
	   if (posclk_rise_det_q)  // MAV (1Jun22) added if (posclk_rise_det) // MAV (21Apr25) added _q
 	     negclk_rise_q  <= `tCQ ~negclk_rise_q;
	end 
   ////////////////////// END CLOCK GATING //////////////////////
     end // always @ (posedge reset or posedge tx_sclk_n)
   
   
   
   // Register Serial Data on Rising Clock Edge
   always @ (posedge clocki)  // MAV (6Apr25) removed posedge reset or (16Apr25) was posedge tx_sclk  (19May25) was negedge
     begin
	if (reset) 
	  sdata_pos_q  <= `tCQ 'h0;
	////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (txClkEn && ~tx_sclk && preMuxed_SCO_se)  begin   // MAV (30Mar25) added if (txClkEn) (16Apr25) added && ~tx_sclk (19May25) added preMuxed_SCO_se
	   if (load)              // MAV (30Mar25) removed else
	     sdata_pos_q  <= `tCQ {pdata[9], pdata[7], pdata[5], pdata[3], pdata[1]};
	   else  
	     sdata_pos_q  <= `tCQ {sdata_pos_q[0],sdata_pos_q[4:1]};
	end
	////////////////////// END CLOCK GATING //////////////////////
      end // always @ (posedge reset or posedge tx_sclk)
   
   
   // Register Serial Data on Negative Clock Edge
   always @ (posedge clocki)  // MAV (6Apr25) removed posedge reset or (16Apr25) was posedge tx_sclk  (19May25) was negedge
     begin
	if (reset) 
	  sdata_neg_q  <= `tCQ 'h0;
	////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (txClkEn && tx_sclk && ~preMuxed_SCO_se) begin   // MAV (30Mar25) added if (txClkEn) (16Apr25) added && tx_sclk (19MNay25) added preMuxed_SCO_se
	   if (load)
	     sdata_neg_q  <= `tCQ {pdata[8], pdata[6], pdata[4], pdata[2], pdata[0]};
	   else  
	     sdata_neg_q  <= `tCQ {sdata_neg_q[1],sdata_neg_q[4:1]};  
	end    
	////////////////////// END CLOCK GATING //////////////////////
     end // always @ (posedge reset or posedge tx_sclk_n)
   

   // Need to delay SDATA by half clock to keep it aligned with TX_SCLK, TX_SCLK_N
   always @ (posedge clocki)  // MAV (6Apr25) removed posedge reset or (16Apr25) was posedge tx_sclk  (19May25) was negedge
     begin
	if (reset) 
	  sdata_neg_zero_q  <= `tCQ 'h0;
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (txClkEn && ~tx_sclk && preMuxed_SCO_se) begin   // MAV (30Mar25) added if (txClkEn) (16Apr25) added && ~tx_sclk (19May25) added preMuxed_SCO_se
	  sdata_neg_zero_q  <= `tCQ sdata_neg_q[0];
	end
   ////////////////////// END CLOCK GATING //////////////////////
     end // always @ (posedge reset or posedge tx_sclk_n)

   
   always @ (posedge clocki)  // MAV (6Apr25) removed posedge reset or (16Apr25) was posedge tx_sclk  (19May25) was negedge
     begin
	if (reset) 
	  sdata_pos_zero_q  <= `tCQ 'h0;
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (txClkEn && tx_sclk && ~preMuxed_SCO_se) begin   // MAV (30Mar25) added if (txClkEn) (16Apr25) added && tx_sclk (19May25) added preMuxed_SCO_se
	  sdata_pos_zero_q  <= `tCQ sdata_pos_q[0];
	end
   ////////////////////// END CLOCK GATING //////////////////////
     end // always @ (posedge reset or posedge tx_sclk_n)
    
  
   // Serialize data by muxing  sdata based on clock states
   assign sdata = (~posclk_rise_q && ~negclk_rise_q)? sdata_pos_zero_q:
	          ( posclk_rise_q && ~negclk_rise_q)? sdata_neg_zero_q:
	          ( posclk_rise_q &&  negclk_rise_q)? sdata_pos_zero_q:
	                                              sdata_neg_zero_q;
  

endmodule
		   
