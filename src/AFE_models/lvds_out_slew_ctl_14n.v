//===========================================================
// Karillon Corporation 2024
// Colorado Springs, Colorado, USA
//===========================================================
// Module      : lvds_out_slew_ctl_14n.v  
// Description : Analog behavioral LVDS TX
// Author      : Rich Klinger
// Date        : Oct. 10, 2024 output
// Note        : This is a temp verilog behavioral model
//               which will be replaced with analog schematic
//===========================================================
`timescale 1ps/1fs
`define USE_IOPADS
`define MIXED_SIGNAL

module lvds_out_slew_ctl_14n
  (`ifdef MIXED_SIGNAL
      inout wire  vddd, vddio, vssio,
   `endif

   // INPUT
   input wire       ctl_in,
   input wire       ctl_inb,
   input wire       slew_0,
   input wire       slew_1,
   // OUTPUT
   output wire      ctl,
   output wire      ctlb 
   );
    
   `ifdef MIXED_SIGNAL
      wire power_good;
      assign power_good = vddd & vddio & ~vssio;
   `endif
    
   ////////////////////////// TX //////////////////////////
  // always @(*)
//	begin
      //assign {ctl,  ctlb} = {ctl_in & ~ctl_inb & power_good,  ~ctl_in & ctl_inb & power_good};
      //assign #0.1 ctl = ctl_in & ~ctl_inb ? 1'b1 : 1'b0;
      //assign #0.1 ctlb = ctl_in & ~ctl_inb ? 1'b0 : 1'b1;
      assign #0.1 ctl = ctl_in & power_good;
      assign #0.1 ctlb = ctl_inb & power_good;
   // end
     //`ifdef MIXED_SIGNAL
     //  assign #0.010 {ctl,  ctlb} = {tx_in & power_good & ~tx_enb,  ~tx_in & power_good & ~tx_enb};
     //`else                            
     //  assign #0.010 {ctl,  ctlb} = {tx_in,  ~tx_in};
     //`endif
   
endmodule