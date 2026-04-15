//===========================================================
// Karillon Corporation 2024
// Colorado Springs, Colorado, USA
//===========================================================
// Module      : lvds_out_lvsft_14n.v  
// Description : Analog behavioral LVDS TX
// Author      : Rich Klinger
// Date        : Oct. 10, 2024 output
// Note        : This is a temp verilog behavioral model
//               which will be replaced with analog schematic
//===========================================================
`timescale 1ps/1fs
`define USE_IOPADS
`define MIXED_SIGNAL

module lvds_out_lvsft_14n
  (`ifdef MIXED_SIGNAL
      inout wire    vddd, vddio, vssd, vssio,
   `endif

   // INPUT
   input wire       d,
   // OUTPUT
   output wire      lsft,
   output wire      lsftb
   );
    
   `ifdef MIXED_SIGNAL
      wire power_good;
      assign power_good = vddd & vddio & ~vssd & ~vssio;
     // assign #0.010 {lsft,  lsftb} = {d & power_good & ~tx_enb,  ~d & power_good & ~tx_enb};
   `endif
    
   ////////////////////////// TX //////////////////////////
   //always @(*)
    //assign #0.010 {lsft,  lsftb} = {d & power_good,  ~d & power_good};
	assign #0.010 lsft = d;
	assign #0.010 lsftb = ~d;
     //`ifdef MIXED_SIGNAL
     //  assign #0.010 {lsft,  lsftb} = {d & power_good & ~tx_enb,  ~d & power_good & ~tx_enb};
     //`else                            
     //  assign #0.010 {lsft,  lsftb} = {d,  ~d};
     //`endif
   
endmodule