//===========================================================
// Karillon Corporation 2024
// Colorado Springs, Colorado, USA
//===========================================================
// Module      : ts_pmu.v  
// Description : Analog behavioral LVDS TX top
// Author      : Rich Klinger
// Date        : Oct. 10, 2024 output
// Note        : This is a temp verilog behavioral model
//               which will be replaced with analog schematic
//===========================================================
`timescale 1ps/1fs
`define USE_IOPADS

module lvds_tx_top
  (`ifdef MIXED_SIGNAL
      input wire vddd, vddio, vss,
      inout wire rto, sns,
   `endif

   // INPUT
   input wire       tx_in,
   input wire       iref,
   // OUTPUT
   output wire [1:0] tx_outp,
   output wire [1:0] tx_outm,
   // CONFIG
   input wire       enb,
   input wire [3:0] tx_offset_trim,
   input wire [1:0] tx_slew,
   // DFT
   output wire      vtop, vbot, ctl, ctlb
   );
    
   ////////////////////////// TX //////////////////////////
   //assign #0.010 {tx_out,  tx_outb}  = {tx_in,  ~tx_in};
   
   wire tx_out_int, tx_outb_int;
   
   lvds_drv_14n LVDS_TX_INST
     (`ifdef MIXED_SIGNAL
	.vddd(vddd), .vddio(vddio), .vssd(vss), .vssio(vss),
      `endif
      // INPUT
      .tx_in(tx_in),
      .iref(iref),
      // OUTPUT
      .tx_out(tx_out_int),
      .tx_outb(tx_outb_int),
      // CONFIG
      .tx_enb(enb),  
      .tx_offset_trim(tx_offset_trim),
      .tx_slew(tx_slew),
      //DFT
      .ctl(ctl), .ctlb(ctlb),
      .vtop(vtop), .vbot(vbot)
   );
   
   PANALOG_18_18_NT_DR_V TXP1(
	  `ifdef MIXED_SIGNAL
	    .AVDD(vddio),
	    .AVSS(vss),
	    .VDD(vddd),
	    .VSS(vss),
	  `endif
	  .PAD(tx_outp[1]),
	  .PADC_IOV(tx_out_int),
	  .PADR1_IOV(),
	  .PADR2_IOV(),
	  .RTO(rto),
	  .SNS(sns)
	);

      PANALOG_18_18_NT_DR_V TXP0(
	  `ifdef MIXED_SIGNAL
	    .AVDD(vddio),
	    .AVSS(vss),
	    .VDD(vddd),
	    .VSS(vss),
	  `endif
	  .PAD(tx_outp[0]),
	  .PADC_IOV(tx_out_int),
	  .PADR1_IOV(),
	  .PADR2_IOV(),
	  .RTO(rto),
	  .SNS(sns)
	);
	
   PANALOG_18_18_NT_DR_V TXM1(
	  `ifdef MIXED_SIGNAL
	    .AVDD(vddio),
	    .AVSS(vss),
	    .VDD(vddd),
	    .VSS(vss),
	  `endif
	  .PAD(tx_outm[1]),
	  .PADC_IOV(tx_outb_int),
	  .PADR1_IOV(),
	  .PADR2_IOV(),
	  .RTO(rto),
	  .SNS(sns)
	);
	
      PANALOG_18_18_NT_DR_V TXM0(
	  `ifdef MIXED_SIGNAL
	    .AVDD(vddio),
	    .AVSS(vss),
	    .VDD(vddd),
	    .VSS(vss),
	  `endif
	  .PAD(tx_outm[0]),
	  .PADC_IOV(tx_outb_int),
	  .PADR1_IOV(),
	  .PADR2_IOV(),
	  .RTO(rto),
	  .SNS(sns)
	);
   
endmodule
