//===========================================================
// Karillon Corporation 2024
// Colorado Springs, Colorado, USA
//===========================================================
// Module      : ts_por.v  
// Description : Analog behavioral PMU
// Author      : Rich Klinger
// Date        : Oct. 10, 2024 output
// Note        : This is a temp verilog behavioral model
//               which will be replaced with analog schematic
//===========================================================
`timescale 1ps/1fs
`define USE_IOPADS

module ts_por
  #(parameter POR_DELAY = 1000000)
  (`ifdef MIXED_SIGNAL
      input wire    vddd, vddio, vssio,
   `endif

   // OUTPUT
   output reg        por

   );
   
   wire all_on;
   assign all_on = 16'hFFFF;
   
   wire power_good;
   assign power_good = vddd & ~vssio;
   
   parameter POR_START_DELAY = (POR_DELAY / 1000) < 1000 ? 1000 : (POR_DELAY / 1000); //RKSW 6/25/2025
   
   ////////////////////////// PMU //////////////////////////
//   initial
//     begin
//	`ifdef MIXED_SIGNAL
//	  por    <= power_good;
//	`else
//	  por    <= 1'b1;
//	`endif
//     end
   
    initial //RKSW 6/25/2025
      por      <= #1 1'b0; //RKSW 6/25/2025
      
    //always @(posedge vddd or negedge vssio) //RKSW - commented out
    always @(posedge vddd) //RKSW
      begin
	`ifdef MIXED_SIGNAL
	  #POR_START_DELAY //RKSW 6/25/2025
	  por      <= #1 1'b1; //RKSW 6/25/2025
	  #POR_DELAY //RKSW
	  por      <= #1 power_good ? 1'b0 : 1'bx; //RKSW
	`else
	  por      <= 1'b0;
	`endif
      end
     
endmodule
