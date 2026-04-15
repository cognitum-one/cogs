//===========================================================
// Karillon Corporation 2024
// Colorado Springs, Colorado, USA
//===========================================================
// Module      : lvds_rx.v  
// Description : Analog behavioral RX
// Author      : Rich Klinger
// Date        : Oct. 10, 2024 output
// Note        : This is a temp verilog behavioral model
//               which will be replaced with analog schematic
//===========================================================
`timescale 1ps/1fs
`define USE_IOPADS

  ////////////////////////// LVDS //////////////////////////
module lvds_rx
  (`ifdef MIXED_SIGNAL
   input wire    vddc, vssc, vddio, vssio,
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
      output reg rx_out,
      output wire rx_outb,
      //DFT
      input wire  [4:0] analog_test,
      input wire  testbus_in_1,
      input wire  testbus_in_2,
      output wire testbus_out_1,
      output wire testbus_out_2,
      output reg testbus_off,
      output reg vref_tg_en,
      input wire  vref_in,
      output wire ampout_p,
      output wire ampout_m,
      output wire compout_p,
      output wire compout_m
   );
   
   wire power_good;
   assign power_good = ~pd_c & vddc & vddio & ~vssc & ~vssio & ibp_50u & ibn_200u;
   
   reg tb_out1, tb_out2;
   //assign testbus_in_1 = tb_in1;
   //assign testbus_in_2 = tb_in2;
   assign testbus_out_1 = tb_out1;
   assign testbus_out_2 = tb_out2;
   assign compout_p = power_good & rx_inp;
   assign compout_m = power_good & rx_inm;
   assign ampout_p = power_good & rx_inp;
   assign ampout_m = power_good & rx_inm;

   always @(*)
     `ifdef MIXED_SIGNAL
       rx_out = #0.010 ( rx_inp && ~rx_inm)?  1'b1 & power_good & ~pd_c:
                       (~rx_inp &&  rx_inm)?  1'b0 & power_good & ~pd_c:
                                                  1'bx;
     `else                            
       rx_out = #0.010 ( rx_inp && ~rx_inm)?  1'b1:
                       (~rx_inp &&  rx_inm)?  1'b0:
                                                  1'bx;
     `endif
     
   assign  #0.01 rx_outb  =  ~rx_out;  // MAV (10Mar25) wasw assign rx_outb = #0.01 rx_out;
     
   //always @({analog_test[4], analog_test[1:0]})
   always @(*)
    case ({analog_test[4], analog_test[1:0]})
      3'b000: begin testbus_off = #0.010 power_good; tb_out1 = #0.010 1'b0;                   tb_out2 = #0.010 1'b0;                   vref_tg_en = #0.010 1'b0; end
      3'b001: begin testbus_off = #0.010 1'b0;       tb_out1 = #0.010 power_good & ampout_p;  tb_out2 = #0.010 power_good & ampout_m;  vref_tg_en = #0.010 1'b0; end
      3'b010: begin testbus_off = #0.010 1'b0;       tb_out1 = #0.010 power_good & compout_p; tb_out2 = #0.010 power_good & compout_m; vref_tg_en = #0.010 1'b0; end
      3'b011: begin testbus_off = #0.010 1'b0;       tb_out1 = #0.010 1'b0;                   tb_out2 = #0.010 1'b0;                   vref_tg_en = #0.010 power_good; end
      3'b100: begin testbus_off = #0.010 1'b0;       tb_out1 = #0.010 power_good & vref_in;   tb_out2 = #0.010 1'b0;                   vref_tg_en = #0.010 1'b0; end
      default: begin testbus_off = #0.010 power_good; tb_out1 = #0.010 1'b0;                   tb_out2 = #0.010 1'b0;                   vref_tg_en = #0.010 1'b0; end
    endcase
    
    /*
    always @(analog_test[4:2])
      if (analog_test[4:2] > 0)
	begin
	  tb_in1 = #0.010 rx_inp;
	  tb_in2 = #0.010 rx_inm;
	end
*/

      
endmodule
