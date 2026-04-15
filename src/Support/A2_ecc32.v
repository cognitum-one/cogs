/* ******************************************************************************************************************************
**
**    Copyright © 1998  Advanced Architectures
**
**    all rights reserved
**    Confidential Information
**    Limited Distribution to authorized Persons Only
**    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
**
**
**    Project Name         : ECC32
**
**    author               : RTT
**    Creation Date        : 1998/03/15
**
**    Description          : 32-bit ECC
**                           Single bit error correction
**                           Double bit error detection
**
****************************************************************************************************************************** */

module   A2_ecc32_dec (
   output wire [31:0]   Do,            // Output data
   output wire  [6:0]   Co,            // Output corrected code bits
   output wire          sec,
   output wire          ded,
   input  wire [38:0]   iD             // Protected input data to be checked and corrected and output as data_out
   );

wire  [38:0]   flip_bit;
wire  [ 6:0]   check, syndrome;
wire           error, parity;

A2_ecc32_gen i1 (
   .e(check[ 6:0]),                // Output  7 bit ecc
   .d(   iD[31:0])                 // Input  32 bit data
   );

assign   syndrome[ 6:0]    =  check[6:0]   ^ iD[38:32];
assign   flip_bit[38:0]    =  dec32(syndrome[6:0]);
assign  {Co[6:0],Do[31:0]} =  iD[38:0] ^ flip_bit[38:0];

// Flag errors
assign   error    =  syndrome != 7'd0;
assign   parity   = ^syndrome;
assign   sec      =  parity   &  error;
assign   ded      = ~parity   &  error;

// ecc syndrome decoder =========================================================================================================

function [38:0] dec32;
input    [ 6:0] syndrome;
begin
dec32 = 39'b0;
case(syndrome[6:0])
   7'b 100_0000: dec32[38] =  1'b1;
   7'b 010_0000: dec32[37] =  1'b1;
   7'b 001_0000: dec32[36] =  1'b1;
   7'b 000_1000: dec32[35] =  1'b1;
   7'b 000_0100: dec32[34] =  1'b1;
   7'b 000_0010: dec32[33] =  1'b1;
   7'b 000_0001: dec32[32] =  1'b1;

   7'b 011_0100: dec32[31] =  1'b1;
   7'b 100_1100: dec32[30] =  1'b1;
   7'b 001_1001: dec32[29] =  1'b1;
   7'b 100_1010: dec32[28] =  1'b1;
   7'b 010_1001: dec32[27] =  1'b1;
   7'b 100_1001: dec32[26] =  1'b1;
   7'b 011_0010: dec32[25] =  1'b1;
   7'b 010_1010: dec32[24] =  1'b1;
   7'b 001_1010: dec32[23] =  1'b1;
   7'b 010_0110: dec32[22] =  1'b1;
   7'b 001_0110: dec32[21] =  1'b1;
   7'b 011_1000: dec32[20] =  1'b1;
   7'b 001_1100: dec32[19] =  1'b1;
   7'b 000_1110: dec32[18] =  1'b1;
   7'b 101_0001: dec32[17] =  1'b1;
   7'b 101_0010: dec32[16] =  1'b1;
   7'b 101_0100: dec32[15] =  1'b1;
   7'b 101_1000: dec32[14] =  1'b1;
   7'b 110_0001: dec32[13] =  1'b1;
   7'b 110_0010: dec32[12] =  1'b1;
   7'b 110_0100: dec32[11] =  1'b1;
   7'b 110_1000: dec32[10] =  1'b1;
   7'b 111_0000: dec32[09] =  1'b1;
   7'b 100_0101: dec32[08] =  1'b1;
   7'b 010_0101: dec32[07] =  1'b1;
   7'b 001_0101: dec32[06] =  1'b1;
   7'b 000_1101: dec32[05] =  1'b1;
   7'b 100_0011: dec32[04] =  1'b1;
   7'b 010_0011: dec32[03] =  1'b1;
   7'b 001_0011: dec32[02] =  1'b1;
   7'b 000_1011: dec32[01] =  1'b1;
   7'b 000_0111: dec32[00] =  1'b1;
   default       dec32     = 39'h0;
   endcase
end
endfunction

endmodule
// ecc generator ================================================================================================================

module  A2_ecc32_gen (
   output wire [ 6:0]   e,
   input  wire [31:0]   d
   );
assign
   e[0] = ^({d[00],d[01],d[02],d[03],d[04],d[05],d[06],d[07],d[08],d[13],d[17],d[26],d[27],d[29]}),
   e[1] = ^({d[00],d[01],d[02],d[03],d[04],d[12],d[16],d[18],d[21],d[22],d[23],d[24],d[25],d[28]}),
   e[2] = ^({d[00],d[05],d[06],d[07],d[08],d[11],d[15],d[18],d[19],d[21],d[22],d[30],d[31]      }),
   e[3] = ^({d[01],d[05],d[10],d[14],d[18],d[19],d[20],d[23],d[24],d[26],d[27],d[28],d[29],d[30]}),
   e[4] = ^({d[02],d[06],d[09],d[14],d[15],d[16],d[17],d[19],d[20],d[21],d[23],d[25],d[29],d[31]}),
   e[5] = ^({d[03],d[07],d[09],d[10],d[11],d[12],d[13],d[20],d[22],d[24],d[25],d[27],d[31]      }),
   e[6] = ^({d[04],d[08],d[09],d[10],d[11],d[12],d[13],d[14],d[15],d[16],d[17],d[26],d[28],d[30]});
endmodule
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
`ifdef   verify_ECC
`timescale 1ns/1ps

//`define   RANDOM_TEST

module t;

parameter   WIDTH = 32;

integer  i,j,pop,even,odd;
reg    [6:0]  num;
reg  [ 31:0]  data_in;       // Input data to be ecc protected and output as ecc_data_out
reg         reset, clock;

wire [ 38:0]  ecc_data;      // Protected output data
wire [ 31:0]  data_out;      // Output data
wire   [6:0]  code_out;      // Output corrected code bits
wire   [6:0]  check;
wire          single;
wire          double;
wire [ 38:0]  ecc_data_out;
reg  [ 38:0]  ecc_data_in;

reg  [ 31:0]  data_in;
reg  [ 38:0]  flip;

// Generate
A2_ecc32_gen mut_g1 (
   .e(ecc_data_out[38:32]), // Output 7 bit ecc
   .d(data_in[31:0])        // Input 32bit data
   );

assign   ecc_data_out[31:0] = data_in[31:0];

// Check and Correct

A2_ecc32_dec mut_d (
   .Do (data_out),            // Output 32 bit Output data
   .Co (code_out),            // Output 7 bit Output corrected code bits
   .sec(single),              // Output Single error corrected event
   .ded(double),              // Output Double error detected  event
   .iD (ecc_data_in[38:0])    // Input  38 bit Protected input data to be checked and corrected and output
   );

initial begin
   clock = 1'b0;
   forever  #50 clock = ~clock;
   end

function [3:0] popc;
input    [8:0] d;
integer  i;
   popc = 0;
   for (i=0;i<9;i=i+1)
      popc = popc + d[i];
endfunction


initial begin
   reset = 1'b0;
   @(posedge clock);
   reset = 1'b1;
   @(posedge clock);
   reset = 1'b0;
   @(posedge clock);
   $display("\n\n No error test");
   data_in  =    32'h0123_4567;
   flip     = 39'h00_0000_0000;
   for (i=0; i<38; i=i+1) begin
      #10   ecc_data_in =  ecc_data_out ^ flip;                               // force a one-bit error
      #10   if (data_out[31:0] != data_in[31:0])   $display(" Error at position %d, %h != %h ",(38-i),data_out[31:0],data_in[31:0]);
            $display("%3d: di = %h, do = %h, flip = %h, syn = %b, iD = %b, iC = %b, sec = %b ded = %b",
              (38-i),data_in, data_out, flip, mut_d.syndrome, ecc_data_in[38:32], mut_d.check, single, double);
      #20   data_in     =  {data_in[0],data_in[31:1]};
      end

   // Single bit error correction
   $display(" Single error correction test");
   data_in  =     32'h0000_0000;
   flip     =  39'h40_0000_0000;
   for (i=0; i<39; i=i+1) begin
      #10   ecc_data_in =  ecc_data_out ^ flip;                               // force a one-bit error
      #10   if (data_out != data_in)   $display(" Error at position %d",(38-i));
            $display("%3d: di = %h, do = %h, flip = %h, syn = %b, iD = %b, iC = %b, sec = %b ded = %b",
              (38-i),
              data_in,
              data_out,
              flip,
              mut_d.syndrome,
              ecc_data_in[38:32],
              mut_d.check,
              single,
              double);
      #20   flip        =  {flip[0],flip[38:1]};
      end

   // Double bit error detection
   $display(" Double error detection test");
   data_in  = 32'h8000_0000;
   flip     = 32'h6000_0000;
   for (i=0; i<32; i=i+1) begin
      #10   ecc_data_in =  ecc_data_out ^ {7'h0,flip};
      repeat(2) @(posedge clock);
      #10   $write("%3d: di = %h, do = %h, flip = %h, syn = %b, iD = %b, iC = %b, sec = %b ded = %b, pop = %h",
              (38-i),
              data_in,
              data_out,
              flip,
              mut_d.syndrome,
              ecc_data_in[38:32],
              mut_d.check,
              single,
              double,
              popc(mut_d.syndrome)
              );
            if (data_out != data_in)   $display(" Error at position %d",(136-i));
            else                       $display(" ");
      repeat(1) @(posedge clock);
      #20   data_in     = {data_in[31],data_in[31:1]};
            flip        = {   flip[31],   flip[31:1]};
      repeat(1) @(posedge clock);
      end
   // Multi bit error
   $display(" Multi-bit errors ");
   data_in  = 32'h8000_0000;
   flip     = 32'h7000_0000;
   for (i=0; i<32; i=i+1) begin
      #10   ecc_data_in =  ecc_data_out ^ {7'h0,flip};
      #10   $write("%3d: di = %h, do = %h, flip = %h, syn = %b, iD = %b, iC = %b, sec = %b ded = %b, pop = %h",
              (38-i),
              data_in,
              data_out,
              flip,
              mut_d.syndrome,
              ecc_data_in[38:32],
              mut_d.check,
              single,
              double,
              popc(mut_d.syndrome)
              );
            if (data_out != data_in)   $display(" Error at position %d",(38-i));
            else                       $display(" ");
      #20   data_in     = {data_in[31],data_in[31:1]};
            flip        = {   flip[31],   flip[31:1]};
      end

`ifdef   RANDOM_TEST
   $display("\n Random testing \n ");

   for (i=0; i<1000_000_000; i=i+1) begin             // Takes a while!!! :(
      data_in  = {$random,$random,$random,$random};
      flip     =  1 <<  (i % 38);
      #10   ecc_data_in =  ecc_data_out ^ flip;
      #10   if (double || !single || i < 10) $display("%3d: di = %h, do = %h, flip = %h, syn = %b, iD = %b, iC = %b, sec = %b ded = %b",
              (38-i),data_in, data_out, flip, mut_d.syndrome, ecc_data_in[38:32], mut_d.check, single, double);
      #20;
      if ((i%100000) == 0) $display("",i);
      end
`endif
//   even = 0;
//   odd  = 0;
//   for (i=0; i<512; i=i+1) begin
//      num = i[8:0];
//      pop = 0;
//      for (j=0;j<9;j=j+1)
//         pop = pop + num[j];
//      if (pop[0] == 1'b0) begin
//         even = even +1;
//         //$display(" Even:  %9b  has %3d bits",num,pop);
//         end
//      else begin
//         odd  = odd  +1;
//         $display(" Odd :  %9b  has %3d bits",num,pop);
//         end
//      end
//   //$display(" Even count is %d",even);
//   $display(" Odd  count is %d",odd);
   //$stop();
   $finish();
   end

endmodule
`endif
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////


