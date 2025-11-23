//===========================================================
// Karillon Corporation 2024
// Colorado Springs, Colorado, USA
//===========================================================
// Module      : ts_pmu.v  
// Description : Analog behavioral PMU
// Author      : Rich Klinger
// Date        : Oct. 10, 2024 output
// Note        : This is a temp verilog behavioral model
//               which will be replaced with analog schematic
//===========================================================
`timescale 1ps/1fs
`define USE_IOPADS

module ts_pmu
  #(parameter PMU_START_DELAY = 100000)
  (`ifdef MIXED_SIGNAL
      input wire vddd, vddio, vssio,
   `endif

   // OUTPUT
   output reg [1:0] ibp_200u,
   output reg [1:0] ibp_50u,
   output reg [1:0] ibn_200u,

   // CONFIG
   input wire        pd,
   input wire  [5:0] pmu_bias_trim,
   // DFT
   input wire        vref_tg_en,
   input wire        testbus_off,
   output wire       vref,
   output wire       rref,
   output wire       vref_out
   );

   reg vref_int;
   assign vref = vref_int;
   assign rref = vref_int;
   assign vref_out = vref_int;
   
   wire [1:0] all_on;
   assign all_on = 2'b11;
   
   wire power_good;
   assign power_good = vddd & vddio & ~vssio;
   
   ////////////////////////// PMU //////////////////////////
   initial
     begin
	ibp_200u <= 2'b00;
	ibp_50u  <= 2'b00;
	ibn_200u <= 2'b00;
	vref_int <= 1'b0;
     end
     
   always @(*)   // MAV (6Mar25) was always @(ps)
    begin
      	#PMU_START_DELAY
	`ifdef MIXED_SIGNAL
	  vref_int <= ~pd & power_good;
	  ibp_200u <= ~pd & power_good ? 2'b11 : 2'b00;
	  ibp_50u  <= ~pd & power_good ? 2'b11 : 2'b00;
	  ibn_200u <= ~pd & power_good ? 2'b11 : 2'b00;
	`else
	  vref_int <= ~pd;
	  ibp_200u <= ~pd ? 2'b11 : 2'b00;
	  ibp_50u  <= ~pd ? 2'b11 : 2'b00;
	  ibn_200u <= ~pd ? 2'b11 : 2'b00;
	`endif
    end
     
endmodule
