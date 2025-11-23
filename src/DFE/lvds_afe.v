//===========================================================
// Karillon Corporation 2024
// Colorado Springs, Colorado, USA
//===========================================================
// Module      : lvds_afe.v  
// Description : Analog top level connectivy for LVDS IO & PMU
// Author      : Mike Vigil
// Date        : Feb 3, 2022 output
//===========================================================
`timescale 1ps/1fs   // MAV (26May22) -added per Roger's suggestion
`define USE_IOPADS   // MAV (24Aug22) -added per Romeo's suggestion

module lvds_afe
  (
// `ifdef MIXED_SIGNAL
   input wire    vddd, vddio, vssd, vssio,
//`endif

   // INPUT
   input wire 	    rx_clk_p,  rx_clk_n,
   input wire       rx_data_p, rx_data_n,
   input wire       tx_clk_se,
   input wire       tx_data_se,
   // OUTPUT
   output reg       por,
   output wire      rx_clk_se, rx_clkb_se,    // MAV (13Dec24) added rx_clkb_se
   output wire      rx_data_se, rx_datab_se,  // MAV (13Dec24) added rx_datab_se
   output wire      tx_clk_p,  tx_clk_n,
   output wire      tx_data_p, tx_data_n,
   // CONFIG
   input wire       pmu_pd,        // MAV (6Mar25) was power_down,
   input wire [5:0] rx_rterm_trim,
   input wire [3:0] rx_offset_trim,
   input wire [4:0] rx_cdac_p,
   input wire [4:0] rx_cdac_m,
   input wire [3:0] rx_bias_trim,
   input wire [3:0] rx_gain_trim,
   input wire [3:0] tx_bias_trim,  
   input wire [3:0] tx_offset_trim,
   input wire [5:0] pmu_bias_trim,
   input wire [4:0] analog_test,
   input wire       analog_reserved,
   // SPARE
   input wire [5:0] lvds_spare_in,
   output wire [7:0] lvds_spare_out);  // MAV (8Apr24) increased from 6 to 8 bits
   
   ////////////////////////// Regs & Wires //////////////////////////
   reg [15:0]      ibp_50u, ibp_200u, ibn_200u;


   ////////////////////////// PMU //////////////////////////
   initial
     begin
	por      <= 1'b1;
	ibp_200u <= 16'h0000;
	ibp_50u  <= 16'h0000;
	ibn_200u <= 16'h0000;
	#100000 
	ibp_200u <= 16'hFFFF;
	ibp_50u  <= 16'hFFFF;
	ibn_200u <= 16'hFFFF;
	#5555   // MAV (3Feb25) delays away from clock edges
	por      <= 1'b0;
	  
     end

// MAV - Behavioral code for now until Rich releases analog schematics //  
   ////////////////////////// TX //////////////////////////
   assign #100 {tx_clk_p,  tx_clk_n}  = {tx_clk_se,  ~tx_clk_se};
   assign #100 {tx_data_p, tx_data_n} = {tx_data_se, ~tx_data_se};

   ////////////////////////// RX //////////////////////////
   assign #100 rx_clk_se = ( rx_clk_p && ~rx_clk_n)?  1'b1:
	       		     (~rx_clk_p &&  rx_clk_n)?  1'b0:
		                                        1'bx;  
   assign rx_clkb_se = ~rx_clk_se; // MAV (13Dec24) added rx_clkb_se
   assign #100 rx_data_se = ( rx_data_p && ~rx_data_n)?  1'b1:
	       		      (~rx_data_p &&  rx_data_n)?  1'b0:
		                                           1'bx;   
   assign rx_datab_se = ~rx_data_se; // MAV (13Dec24) added rx_datab_se
   

   // DEBUG
   assign lvds_spare_out = 8'b00110011;
   wire [55:0]   debug = {analog_reserved,analog_test,lvds_spare_in,pmu_bias_trim,
			  tx_offset_trim,tx_bias_trim,rx_gain_trim,rx_bias_trim,
			  rx_cdac_m,rx_cdac_p,rx_offset_trim,rx_rterm_trim,pmu_pd};  // was power_down
   
   
endmodule 

 
// MAV - commented out for now un6til Rich releases analog schematics //  
   ////////////////////////// PMU //////////////////////////
 /*  lvds_pmu pmu1
     (`ifdef MIXED_SIGNAL
      .vddio(vddio),
      .vssio(vssio),
      `endif
      // INPUT 
      .pd_pmu(power_down),
      // CONFIG       
      .pmu_bias_trim(pmu_bias_trim),
      // OUTPUT
      .porb(porb),
      .ibp_50u(ibp_50u),
      .ibp_200u(ibp_200u),
      .ibp_200u(ibp_200u)
      );
   
   ////////////////////////// LVDS //////////////////////////
    // LVDS CLOCK IN
   lvds_rx LVDS_CLKIN_INST
     (`ifdef MIXED_SIGNAL
      .vddio(vddio),
      .vssio(vssio),
      .vddc(vddd),
      .vssc(vssd),
      `endif
      // BIAS 
      .ibp_50u       (rx_clk_ibp_50u),
      .ibn_200u      (rx_clk_ibn_200u), 
      // INPUT
      .pd_rx         (power_down),
      .rx_inp        (),
      .rx_inn        (),
    

      .delay_p       (rx_clk_delay_p),
      .delay_m       (rx_clk_delay_m),
      .pd            (rx_pd),
      .pd_c          (rx_pd),
      .rx_inp        (rx_clk_inpi),
      .rx_inm        (rx_clk_inmi),
      .trim_c        (rx_clk_trim_c), 
      .outp          (rx_clk_SCI),
      .outn          (),  // UNUSED 
      ); //lvds_rx_clk
   
   PANALOG_18_18_NT_DR_V I15(
	  `ifdef MIXED_SIGNAL
	    .AVDD(vddio),
	    .AVSS(vssio),
	    .VDD(vddd),
	    .VSS(vssd),
	  `endif
	  .PAD(rx_clk_inp),
	  .PADC_IOV(rx_clk_inpi),
	  .PADR1_IOV(),
	  .PADR2_IOV(),
	  .RTO(rto),
	  .SNS(sns)
	);
	
   PANALOG_18_18_NT_DR_V I14(
	  `ifdef MIXED_SIGNAL
	    .AVDD(vddio),
	    .AVSS(vssio),
	    .VDD(vddd),
	    .VSS(vssd),
	  `endif
	  .PAD(rx_clk_inm),
	  .PADC_IOV(rx_clk_inmi),
	  .PADR1_IOV(),
	  .PADR2_IOV(),
	  .RTO(rto),
	  .SNS(sns)
	);

/////////////////////////////////////////////////////////////
// LVDS DATA IN
   lvds_rx LVDS_DATAIN_INST
     (`ifdef MIXED_SIGNAL
      .vddio(vddio),
      .vssio(vssio),
      .vddc(vddd),
      .vssc(vssd),
      `endif
       // BIAS 
      .ibn_200u      (rx_data_ibn_200u), 
      .ibp_50u       (rx_data_ibp_50u),
      // INPUT

      
      .delay_p       (rx_data_delay_p),
      .delay_m       (rx_data_delay_m),
      .pd            (rx_pd),
      .pd_c          (rx_pd),
      .rx_inp        (rx_data_inpi),
      .rx_inm        (rx_data_inmi),
      .trim_c        (rx_data_trim_c),
      .outp          (rx_data_SDI),  // MAV (16Jun22) corrected placement of (
      .outn          (), // UNUSED
      .ampout_p      (),  // UNUSED 
      .ampout_n      (),  // UNUSED 
      .compout_p     (),  // UNUSED 
      .compout_n     ()   // UNUSED 
      );
*/
   ////////////////////////// Power / Gnd Pads //////////////////////////
 /* MAV - Commented out 
    PVDD_08_08_NT_DR_V vddd_pad1(
	  `ifdef MIXED_SIGNAL
	    .AVDD(vddio),
	    .AVSS(vssio),
	    .VDD(vddd),
	    .VSS(vssd),
	  `endif
	  .RTO(rto),
	  .SNS(sns)
	);
	
   PDVDD_18_18_NT_DR_V vddio_pad1(
	  `ifdef MIXED_SIGNAL
	    .AVDD(vddio),
	    .AVSS(vssio),
	    .VDD(vddd),
	    .VSS(vssd),
	  `endif
	  .RTO(rto),
	  .SNS(sns)
	);
	
	PVSS_08_08_NT_DR_V vssd_pad1(
	  `ifdef MIXED_SIGNAL
	    .AVDD(vddio),
	    .AVSS(vssio),
	    .VDD(vddd),
	    .VSS(vssd),
	  `endif
	  .RTO(rto),
	  .SNS(sns)
	);
	
	PDVSS_18_18_NT_DR_V vssio_pad1(
	  `ifdef MIXED_SIGNAL
	    .AVDD(vddio),
	    .AVSS(vssio),
	    .VDD(vddd),
	    .VSS(vssd),
	  `endif
	  .RTO(rto),
	  .SNS(sns)
	);
*/ 

