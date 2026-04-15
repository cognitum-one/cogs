//===========================================================
// Karillon Corporation 2024
// Colorado Springs, Colorado, USA
//===========================================================
// Module      : lvds_out_14n.v  
// Description : Analog behavioral LVDS TX
// Author      : Rich Klinger
// Date        : Oct. 10, 2024 output
// Note        : This is a temp verilog behavioral model
//               which will be replaced with analog schematic
//===========================================================
`timescale 1ps/1fs
`define USE_IOPADS
`define MIXED_SIGNAL

module lvds_out_14n
  (`ifdef MIXED_SIGNAL
      inout wire    vddd, vddio, vssd, vssio,
   `endif

   // INPUT
   input wire       enhv,
   input wire       ctl,
   input wire       ctlb,
   input wire       enhvb,
   input wire [3:0] tx_offset_trim,
   inout wire       iref,
   // OUTPUT
   output wire      tx_out,
   output wire      tx_outb,
   // DFT
   inout wire       vtop, vbot,
   inout wire       pb1, vnb
   
   );
    
   `ifdef MIXED_SIGNAL
      wire power_good;
      assign power_good = vddd & vddio & ~vssd & ~vssio & iref;
      //assign #0.010 {tx_out,  tx_outb} = {tx_in & power_good & enhv & ~enhvb,  ~tx_in & power_good & enhv & ~enhvb};
   `endif
    
   ////////////////////////// TX //////////////////////////
  // always @(*)
    assign #0.010 {tx_out,  tx_outb} = {ctl & ~ctlb & power_good & enhv & ~enhvb,  ~ctl & ctlb & power_good & enhv & ~enhvb};
   //  `ifdef MIXED_SIGNAL
   //    assign #0.010 {tx_out,  tx_outb} = {ctl & ~ctlb & power_good & enhv & ~enhvb,  ~ctl & ctlb & power_good & enhv & ~enhvb};
   //  `else                            
   //    assign #0.010 {tx_out,  tx_outb} = {ctl & ~ctlb,  ~ctl & ctlb};
   //  `endif
   
endmodule