//===========================================================
// Karillon Corporation 2024
// Colorado Springs, Colorado, USA
//===========================================================
// Module      : lvl_sft_sl_1p8_14n.v  
// Description : Analog behavioral LVDS TX
// Author      : Rich Klinger
// Date        : Oct. 10, 2024 output
// Note        : This is a temp verilog behavioral model
//               which will be replaced with analog schematic
//===========================================================
`timescale 1ps/1fs
`define USE_IOPADS

module lvl_sft_dvr_sl_1p8_14n
  (`ifdef MIXED_SIGNAL
      inout wire    vddd, vddio, vssd, 
   `endif

   // INPUT
   input wire       d,
   input wire       enb,
   // OUTPUT
   output wire      d_lsft,
   output wire      d_lsftb
   );
    
   `ifdef MIXED_SIGNAL
      wire power_goodXYZ;
      assign power_goodXYZ = vddd & vddio & ~vssd;
   `endif
    
   ////////////////////////// TX //////////////////////////
  // always @(*)
    assign #0.010 d_lsft =  d & ~enb;
    assign #0.010 d_lsftb = ~d & ~enb;
     //`ifdef MIXED_SIGNAL
     //  assign #0.010 {tx_out,  tx_outb} = {tx_in & power_good & ~tx_enb,  ~tx_in & power_good & ~tx_enb};
     //`else                            
     //  assign #0.010 {tx_out,  tx_outb} = {tx_in,  ~tx_in};
     //`endif
   
endmodule