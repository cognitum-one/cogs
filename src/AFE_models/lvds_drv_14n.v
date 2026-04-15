//===========================================================
// Karillon Corporation 2024
// Colorado Springs, Colorado, USA
//===========================================================
// Module      : ts_pmu.v  
// Description : Analog behavioral LVDS TX
// Author      : Rich Klinger
// Date        : Oct. 10, 2024 output
// Note        : This is a temp verilog behavioral model
//               which will be replaced with analog schematic
//===========================================================
`timescale 1ps/1fs
`define USE_IOPADS

module lvds_drv_14n
  (`ifdef MIXED_SIGNAL
      inout wire    vddd, vddio, vssd, vssio,
   `endif

   // INPUT
   input wire       tx_in,
   input wire       iref,
   // OUTPUT
   output wire      tx_out,
   output wire      tx_outb,
   // CONFIG
   input wire       tx_enb,  
   input wire [3:0] tx_offset_trim,
   input wire [1:0] tx_slew,
   //DFT
   output wire      ctl, ctlb,
   output wire      vtop, vbot
   );
    
  wire en, d_lsft, d_lsftb, pb1, vnb;

  wire power_good;
  assign power_good = vddd & vddio & ~vssd & ~vssio & iref;
  
  assign en = ~tx_enb & power_good;
  
  //assign ctl  = power_good;
  //assign ctlb = power_good;
  assign vtop = power_good;
  assign vbot = power_good;

lvds_out_lvsft_14n ls1(
	// Power/Gnd
    .vddd(vddd),
    .vddio(vddio), 
    .vssd(vssd),
    .vssio(vssio),
    // INPUT
	.d(en),
	// OUTPUT
	.lsft(enhv),
	.lsftb(enhvb)
);

lvl_sft_dvr_sl_1p8_14n ls2(
	// Power/Gnd
    .vddd(vddd),
    .vddio(vddio), 
    .vssd(vssd),
    // INPUT
	.d(tx_in),
	.enb(tx_enb),
	// OUTPUT
	.d_lsft(lsft),
	.d_lsftb(lsftb)
);

lvds_out_slew_ctl_14n slew1(
	// Power/Gnd
    .vddd(vddd),
    .vddio(vddio), 
    .vssio(vssio),
     // INPUT
	.ctl_in(lsft),
	.ctl_inb(lsftb),
	.slew_0(tx_slew[0]),
	.slew_1(tx_slew[1]),
	// OUTPUT
	.ctl(ctl),
	.ctlb(ctlb)
);

lvds_out_14n I_lvds_out(
	// Power/Gnd
    .vddd(vddd),
    .vddio(vddio), 
    .vssd(vssd),
    .vssio(vssio),
    .iref(iref),
     // INPUT
	.enhv(enhv),
	.ctl(ctl),
	.ctlb(ctlb),
	.enhvb(enhvb),
	.tx_offset_trim(tx_offset_trim),
	// OUTPUT
	.tx_out(tx_out),
	.tx_outb(tx_outb),
	// DFT
	.vtop(vtop),
	.vbot(vbot),
	.pb1(pb1),
	.vnb(vnb)

);

endmodule