//===========================================================
// Karillon Corporation 2024
// Colorado Springs, Colorado, USA
//===========================================================
// Module      : lvds_rx_top.v  
// Description : Analog behavioral RX + Pads
// Author      : Rich Klinger
// Date        : Oct. 10, 2024 output
// Note        : This is a temp verilog behavioral model
//               which will be replaced with analog schematic
//===========================================================
`timescale 1ps/1fs
`define USE_IOPADS

  ////////////////////////// LVDS //////////////////////////
module lvds_rx_top
  (`ifdef MIXED_SIGNAL
      input wire vddc, vddio, vss,
      inout wire rto, sns,
   `endif

      // BIAS 
      input wire ibp_50u,
      input wire ibn_200u, 
      // INPUT
      input wire rx_inp,
      input wire rx_inm,
      // CONFIG
      input wire [4:0] rx_cdac_p,
      input wire [4:0] rx_cdac_m,
      input wire [3:0] rx_bias_trim,
      input wire [5:0] rx_rterm_trim,
      input wire pd_c,
      input wire trim_c, 
      // OUTPUT
      output wire rx_out,
      output wire rx_outb,
      //DFT
      input wire  [4:0] analog_test,
      input wire  testbus_in_1,
      input wire  testbus_in_2,
      output wire testbus_out_1,
      output wire testbus_out_2,
      output wire testbus_off,
      output wire vref_tg_en,
      input wire  vref_in,
      output wire ampout_p,
      output wire ampout_m,
      output wire compout_p,
      output wire compout_m
   );
   
   
   
 lvds_rx LVDS_RX_INST
     (`ifdef MIXED_SIGNAL
        .vddio(vddio),
        .vssio(vss),
        .vddc(vddc),
        .vssc(vss),
      `endif
      // BIAS 
      .ibp_50u        (ibp_50u),
      .ibn_200u       (ibn_200u), 
      // INPUT
      .rx_inp         (rx_inpi),
      .rx_inm         (rx_inmi),
      // CONFIG
      .rx_cdac_p      (rx_cdac_p),
      .rx_cdac_m      (rx_cdac_m),
      .rx_bias_trim   (rx_bias_trim),
      .rx_rterm_trim  (rx_rterm_trim),
      .pd_c           (pd_c),
      .trim_c         (trim_c), 
      // OUTPUT
      .rx_out           (rx_out),
      .rx_outb          (rx_outb),
      //DFT
      .analog_test  (analog_test),
      .testbus_in_1  (testbus_in_1),
      .testbus_in_2  (testbus_in_2),
      .testbus_out_1 (testbus_out_1),
      .testbus_out_2 (testbus_out_2),
      .testbus_off  (testbus_off),
      .vref_tg_en   (vref_tg_en),
      .vref_in      (vref_in),
      .ampout_p     (ampout_p),
      .ampout_m     (ampout_m),
      .compout_p    (compout_p),
      .compout_m    (compout_m)
      );
   
   PANALOG_18_18_NT_DR_V RXP(
	  `ifdef MIXED_SIGNAL
	    .AVDD(vddio),
	    .AVSS(vss),
	    .VDD(vddc),
	    .VSS(vss),
	  `endif
	  .PAD(rx_inp),
	  .PADC_IOV(rx_inpi),
	  .PADR1_IOV(),
	  .PADR2_IOV(),
	  .RTO(rto),
	  .SNS(sns)
	);
	
   PANALOG_18_18_NT_DR_V RXM(
	  `ifdef MIXED_SIGNAL
	    .AVDD(vddio),
	    .AVSS(vss),
	    .VDD(vddc),
	    .VSS(vss),
	  `endif
	  .PAD(rx_inm),
	  .PADC_IOV(rx_inmi),
	  .PADR1_IOV(),
	  .PADR2_IOV(),
	  .RTO(rto),
	  .SNS(sns)
	);

endmodule