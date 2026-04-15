//===========================================================
// Karillon Corporation 2021
// Colorado Springs, Colorado, USA
//===========================================================
// Module      : lvds_sipo.v  
// Description : Serial to Parallel (Deserializer)
// Author      : Mike Vigil
// Date        : Feb 3, 2022   // May 1, 2025
//===========================================================

`ifdef  synthesis //JLSW
   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh"
`else
   `include "A2_project_settings.vh"
`endif

//`timescale 1ps/1fs   // MAV (26May22) -added per Roger's suggestion //JLSW

module lvds_sipo
  (
   input wire 	     rx_sclk, // high speed serial clock  
   input wire 	     rx_sclk_n, // Inverted high speed serial clock  - MAV (21Feb22)
   input wire 	     rxClkEn, // MAV (31Mar25) added rxClkEn port
   input wire 	     reset, // active high reset
   input wire 	     capture, // Parallel Capture
   input wire 	     sdata, // serial data 
   output wire 	     rx_idle_det, // // Idle detected to reset capture counter
   output reg [ 9:0] pdata // parallel data 
   );
                    

   ////////////////////////// Regs & Wires //////////////////////////
   reg 		       posedge_clk_data, negedge_clk_data; // even & odd sampled data
   reg [4:0] 	       sr_posedge_clk_data, sr_negedge_clk_data, sr_negedge_clk_data_q;// even/odd data shift register
   wire [9:0] 	       sdata_msb2lsb_pos_neg;  // Serial data concatenated from Pos then Neg from MSB to LSB 
   reg [9:0] 	       sdata_msb2lsb_pos_neg_q;  // Registered version of sdata_msb2lsb_pos_neg 
   // MAVV (26Apr25) New logic for negative clock sampling data first 
   reg [4:0] 	       sr_posedge_clk_data_q;// even/odd data shift register
   wire [9:0] 	       sdata_msb2lsb_neg_pos;  // Serial data concatenated from Neg then Pos from MSB to LSB 
   wire 	       rx_idle_det_pos, rx_idle_det_neg;  // IDLE detect for Neg-Pos order
   reg 		       idle_det_pos_q; // IDLE Detect flopped for Data Muxing
   reg [9:0] 	       sdata_msb2lsb_neg_pos_q;  // Registered version of sdata_msb2lsb_pos_neg 
   
   ////////////////////////// Flop SDATA on Both Clock Edges //////////////////////////
   // Positive Edge clock capture of SDATA 
   always @ (posedge rx_sclk)  // MAV (6Apr25) removed  posedge reset or
     begin
	if (reset)
	  begin
             posedge_clk_data  <= `tCQ 1'h0;
	  end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (rxClkEn) // MAV (31Mar25) added if (rxClkEn)
	  begin
	     posedge_clk_data  <= `tCQ sdata;
	  end    
   ////////////////////// END CLOCK GATING //////////////////////
     end   
 
   // Negative  Edge clock capture of SDATA 
   always @ (posedge rx_sclk_n) // MAV (6Apr25) removed posedge reset or 
     begin
	if (reset)
	  begin
             negedge_clk_data  <= `tCQ 1'h0;
	  end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (rxClkEn) // MAV (31Mar25) added if (rxClkEn)
	  begin
	     negedge_clk_data  <= `tCQ sdata;
	  end    
   ////////////////////// END CLOCK GATING //////////////////////
     end    



   ////////////////////////// Shift Regs //////////////////////////
   // Negative Clock edge
   always @ (posedge rx_sclk_n) // MAV (6Apr25) removed posedge reset  or 
     begin
	if (reset)
	  begin
	     sr_negedge_clk_data <= `tCQ 5'h0;
	  end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (rxClkEn) // MAV (31Mar25) added if (rxClkEn)
	  begin
	     sr_negedge_clk_data <= `tCQ {negedge_clk_data,sr_negedge_clk_data[4:1]};
	  end    
   ////////////////////// END CLOCK GATING //////////////////////
     end   

   // Positive Clock edge
   always @ (posedge rx_sclk) // MAV (6Apr25) removed posedge reset  or 
     begin
	if (reset)
	  begin
             sr_posedge_clk_data   <= `tCQ 5'h0;
	     sr_negedge_clk_data_q <= `tCQ 5'h0;	  
	       end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (rxClkEn) // MAV (31Mar25) added if (rxClkEn)
	  begin
	     sr_posedge_clk_data   <= `tCQ {posedge_clk_data, sr_posedge_clk_data[4:1]};
	     sr_negedge_clk_data_q <= `tCQ sr_negedge_clk_data; // Register to Positive Clock edge
	  end    
   ////////////////////// END CLOCK GATING //////////////////////
     end   



   // Concatenate Shift Reg data from MSB to LSB starting with 
   // Positive Clk Data followed by Negative Clock Data. 
   assign sdata_msb2lsb_pos_neg = {sr_posedge_clk_data[4],sr_negedge_clk_data_q[4],
				   sr_posedge_clk_data[3],sr_negedge_clk_data_q[3],
				   sr_posedge_clk_data[2],sr_negedge_clk_data_q[2],
				   sr_posedge_clk_data[1],sr_negedge_clk_data_q[1],
				   sr_posedge_clk_data[0],sr_negedge_clk_data_q[0]};

   
   // Need to register before capturing 
   always @ (posedge rx_sclk) // MAV (6Apr25) removed posedge reset  or 
     begin
	if (reset)
	  begin
	     sdata_msb2lsb_pos_neg_q <= `tCQ 10'h0;	  
	  end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (rxClkEn) // MAV (31Mar25) added if (rxClkEn)
	  begin
	     sdata_msb2lsb_pos_neg_q <= `tCQ sdata_msb2lsb_pos_neg;
	  end    
     end   


   // Detect Positive or Negative Disparity IDLE   
   assign rx_idle_det_pos = ((sdata_msb2lsb_pos_neg == 10'h17C) ||  // IDLE Positive Disparity  // MAV (26Apr25) added _pos
			 (sdata_msb2lsb_pos_neg == 10'h283));   // IDLE Negative Disparity
   
   // Moved Capture counter and assign logic to lvds_control    

     
   // Capture Parallel Data
   always @ (posedge rx_sclk)  // MAV (6Apr25) removed  posedge reset or
     begin
	if (reset)
	  begin
             pdata  <= `tCQ 10'h00;      
	  end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (rxClkEn) begin// MAV (31Mar25) added if (rxClkEn)
	   if (capture) // MAV (31Mar25) removed else
	     begin
		pdata  <= `tCQ (idle_det_pos_q)? sdata_msb2lsb_pos_neg_q : sdata_msb2lsb_neg_pos_q; 
       	     end
	end
   ////////////////////// END CLOCK GATING //////////////////////
     end // always @ (posedge reset or posedge rx_sclk)
   
/////////////////////////////////////////////////////////////////////////
//  MAV (26Apr25) New logic for negative clock sampling data first 
/////////////////////////////////////////////////////////////////////////

    
   // Need to flop  sr_posedge_clk_data for Neg - Pos data munging
   always @ (posedge rx_sclk_n)
     begin
	if (reset)
	  begin
	     sr_posedge_clk_data_q  <= `tCQ 5'h0;	  
	       end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (rxClkEn)
	  begin
	     sr_posedge_clk_data_q  <= `tCQ sr_posedge_clk_data;	  
	  end    
   ////////////////////// END CLOCK GATING //////////////////////
     end // always @ (posedge rx_sclk_n)
   
   // New ordering for Neg then Pos
   assign sdata_msb2lsb_neg_pos = {sr_negedge_clk_data[4],sr_posedge_clk_data_q[4],
				   sr_negedge_clk_data[3],sr_posedge_clk_data_q[3],
				   sr_negedge_clk_data[2],sr_posedge_clk_data_q[2],
				   sr_negedge_clk_data[1],sr_posedge_clk_data_q[1],
				   sr_negedge_clk_data[0],sr_posedge_clk_data_q[0]};
    
   // Need to flop  sr_posedge_clk_data for Neg - Pos data munging
   assign rx_idle_det_neg = ((sdata_msb2lsb_neg_pos == 10'h17C) ||  // IDLE Positive Disparity
			     (sdata_msb2lsb_neg_pos == 10'h283));   // IDLE Negative Disparity

   // Two IDLE Detections are ORef
   assign  rx_idle_det = rx_idle_det_pos || rx_idle_det_neg;
   

   
   // IDLE Detect flopped for Data Muxing
   always @ (posedge rx_sclk) 
     begin
	if (reset)
	  begin
            idle_det_pos_q   <= `tCQ 1'h1;      
	  end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (rxClkEn) begin
	   if (rx_idle_det_pos) 
	     begin
		idle_det_pos_q   <= `tCQ 1'h1; 
       	     end
	   else if (rx_idle_det_neg) 
	     begin
		idle_det_pos_q   <= `tCQ 1'h0; 
       	     end
	end
   ////////////////////// END CLOCK GATING //////////////////////
     end // always @ (posedge rx_sclk)
   

   // Need to register before capturing 
   always @ (posedge rx_sclk) 
     begin
	if (reset)
	  begin
	     sdata_msb2lsb_neg_pos_q <= `tCQ 10'h0;	  
	  end
   ////////////////////// BEGIN CLOCK GATING //////////////////////
	else if (rxClkEn) 
	  begin
	     sdata_msb2lsb_neg_pos_q <= `tCQ sdata_msb2lsb_neg_pos;
	  end    
    ////////////////////// END CLOCK GATING //////////////////////
    end // always @ (posedge rx_sclk)



  
endmodule

// MAV//   reg [9:0] pipeline_msb2lsb;
// MAV//
// MAV//   always @ (rx_sclk)
// MAV//     if (reset)
// MAV//       begin 
// MAV//	  pipeline_msb2lsb <= #2 0;
// MAV//       end
// MAV//     else
// MAV//       begin 
// MAV//	  pipeline_msb2lsb <= #2 {sdata,pipeline_msb2lsb[9:1]};
// MAV//       end
// MAV//
// MAV//  DEBUG 
// MAV//    wire [9:0] pipePosNeg, pipeNegPos;
// MAV//   assign pipePosNeg = {sr_posedge_clk_data[4], sr_negedge_clk_data[4],
// MAV//			sr_posedge_clk_data[3], sr_negedge_clk_data[3],
// MAV//			sr_posedge_clk_data[2], sr_negedge_clk_data[2],
// MAV//			sr_posedge_clk_data[1], sr_negedge_clk_data[1],
// MAV//			sr_posedge_clk_data[0], sr_negedge_clk_data[0]};
// MAV//
// MAV//   assign pipeNegPos = {sr_negedge_clk_data[4], sr_posedge_clk_data[4],
// MAV//			sr_negedge_clk_data[3], sr_posedge_clk_data[3],
// MAV//			sr_negedge_clk_data[2], sr_posedge_clk_data[2],
// MAV//			sr_negedge_clk_data[1], sr_posedge_clk_data[1],
// MAV//			sr_negedge_clk_data[0], sr_posedge_clk_data[0]};
 

 
