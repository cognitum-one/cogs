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
**    Project Name         : ECC128
**
**    author               : RTT
**    Creation Date        : 1998/03/15
**
**    Description          : 128-bit ECC
**                           Single bit error correction
**                           Double bit error detection
**                           Encode minimized by using all 1-bit, all 3-bit, and the remaining are 5-bit syndromes,
**
**    Prototypes:
**       A2_ECC128_dec (
**          .Do   (),      // Output 128 bit Output data
**          .Co   (),      // Output 9 bit Output corrected code bits
**          .sec  (),      // Output Single error corrected event
**          .ded  (),      // Output Double error detected  event
**          .iD   (),      // Input 136 bit Protected input data to be checked and corrected and output
**          .reset(),
**          .clock()
**          );
**
**       A2_ECC128_gen (
**          .e(),       // Output 9 bit ecc
**          .d(),       // Input 128 bit data
**          );
**
****************************************************************************************************************************** */

module   A2_ECC128_dec (
   output wire [127:0]  Do,      // Output data
   output wire   [8:0]  Co,      // Output corrected code bits
   output wire          sec,     // Single error corrected event
   output wire          ded,     // Double error detected  event
   input  wire [136:0]  iD,      // Protected input data to be checked and corrected and output
   input  wire          reset,
   input  wire          clock
   );

(* siart: RETIME  *) reg     [136:0]   ppl_1,ppl_2;
(* siart: RETIME  *) always @(posedge clock) ppl_1 <= #1 iD;
(* siart: RETIME  *) always @(posedge clock) ppl_2 <= #1 ppl_1;

wire    [136:0]   flip_bit;
wire      [8:0]   check, syndrome;
wire              error, parity;

A2_ECC128_gen i1 (
   .e(check[8:0]),               // Output 9 bit ecc
   .d(ppl_2[127:0])                 // Input 128 bit data
   );

assign   syndrome[  8:0]      =  check[8:0] ^ ppl_2[136:128];
assign   flip_bit[136:0]      =  dec128(syndrome[8:0]);
assign  {Co[8:0],Do[127:0]}   =  iD[136:0]  ^ flip_bit[136:0];

// Flag errors
assign   error    =  syndrome != 0;
assign   parity   = ^syndrome;
assign   sec      =  parity   &  error;
assign   ded      = ~parity   &  error;

// ECC SYNDROME DECODER ========================================================================================================

function [136:0] dec128;
input    [  8:0] syndrome;
begin
dec128 = 137'b0;
case(syndrome[8:0])                     // RTT20240324 fixed 121 -- syndrome was the same as 127!
   9'b100000000: dec128[136] = 1'b1;    9'b010111010: dec128[127] = 1'b1;    9'b111000011: dec128[111] = 1'b1;
   9'b010000000: dec128[135] = 1'b1;    9'b110010110: dec128[126] = 1'b1;    9'b110100011: dec128[110] = 1'b1;
   9'b001000000: dec128[134] = 1'b1;    9'b101101001: dec128[125] = 1'b1;    9'b110010011: dec128[109] = 1'b1;
   9'b000100000: dec128[133] = 1'b1;    9'b100101101: dec128[124] = 1'b1;    9'b110001011: dec128[108] = 1'b1;
   9'b000010000: dec128[132] = 1'b1;    9'b010110011: dec128[123] = 1'b1;    9'b110000111: dec128[107] = 1'b1;
   9'b000001000: dec128[131] = 1'b1;    9'b010110110: dec128[122] = 1'b1;    9'b111010001: dec128[106] = 1'b1;
   9'b000000100: dec128[130] = 1'b1;    9'b010111001: dec128[121] = 1'b1;    9'b111010010: dec128[105] = 1'b1;
   9'b000000010: dec128[129] = 1'b1;    9'b011011010: dec128[120] = 1'b1;    9'b111010100: dec128[104] = 1'b1;
   9'b000000001: dec128[128] = 1'b1;    9'b110011010: dec128[119] = 1'b1;    9'b111011000: dec128[103] = 1'b1;
                                        9'b101001101: dec128[118] = 1'b1;    9'b100010111: dec128[102] = 1'b1;
                                        9'b101010101: dec128[117] = 1'b1;    9'b010010111: dec128[101] = 1'b1;
                                        9'b101100101: dec128[116] = 1'b1;    9'b001010111: dec128[100] = 1'b1;
                                        9'b001101101: dec128[115] = 1'b1;    9'b000110111: dec128[099] = 1'b1;
                                        9'b001101110: dec128[114] = 1'b1;    9'b100111100: dec128[098] = 1'b1;
                                        9'b101101100: dec128[113] = 1'b1;    9'b010111100: dec128[097] = 1'b1;
                                        9'b011101100: dec128[112] = 1'b1;    9'b001111001: dec128[096] = 1'b1;
   9'b001111010: dec128[095] = 1'b1;    9'b100000011: dec128[079] = 1'b1;    9'b010010001: dec128[063] = 1'b1;
   9'b001111100: dec128[094] = 1'b1;    9'b001000011: dec128[078] = 1'b1;    9'b100010001: dec128[062] = 1'b1;
   9'b111100001: dec128[093] = 1'b1;    9'b000100011: dec128[077] = 1'b1;    9'b001100001: dec128[061] = 1'b1;
   9'b111100010: dec128[092] = 1'b1;    9'b000001101: dec128[076] = 1'b1;    9'b010100001: dec128[060] = 1'b1;
   9'b111100100: dec128[091] = 1'b1;    9'b000010101: dec128[075] = 1'b1;    9'b100100001: dec128[059] = 1'b1;
   9'b111101000: dec128[090] = 1'b1;    9'b000100101: dec128[074] = 1'b1;    9'b011000001: dec128[058] = 1'b1;
   9'b111110000: dec128[089] = 1'b1;    9'b001000101: dec128[073] = 1'b1;    9'b101000001: dec128[057] = 1'b1;
   9'b100001111: dec128[088] = 1'b1;    9'b100000101: dec128[072] = 1'b1;    9'b110000001: dec128[056] = 1'b1;
   9'b010001111: dec128[087] = 1'b1;    9'b010000101: dec128[071] = 1'b1;    9'b000001110: dec128[055] = 1'b1;
   9'b001001111: dec128[086] = 1'b1;    9'b000011001: dec128[070] = 1'b1;    9'b000010110: dec128[054] = 1'b1;
   9'b000101111: dec128[085] = 1'b1;    9'b000101001: dec128[069] = 1'b1;    9'b000100110: dec128[053] = 1'b1;
   9'b000011111: dec128[084] = 1'b1;    9'b001001001: dec128[068] = 1'b1;    9'b001000110: dec128[052] = 1'b1;
   9'b000000111: dec128[083] = 1'b1;    9'b100001001: dec128[067] = 1'b1;    9'b100000110: dec128[051] = 1'b1;
   9'b000001011: dec128[082] = 1'b1;    9'b010001001: dec128[066] = 1'b1;    9'b010000110: dec128[050] = 1'b1;
   9'b000010011: dec128[081] = 1'b1;    9'b000110001: dec128[065] = 1'b1;    9'b000011010: dec128[049] = 1'b1;
   9'b010000011: dec128[080] = 1'b1;    9'b001010001: dec128[064] = 1'b1;    9'b000101010: dec128[048] = 1'b1;
   9'b001001010: dec128[047] = 1'b1;    9'b010001100: dec128[031] = 1'b1;    9'b001101000: dec128[015] = 1'b1;
   9'b010001010: dec128[046] = 1'b1;    9'b001001100: dec128[030] = 1'b1;    9'b010101000: dec128[014] = 1'b1;
   9'b100001010: dec128[045] = 1'b1;    9'b000110100: dec128[029] = 1'b1;    9'b100101000: dec128[013] = 1'b1;
   9'b001010010: dec128[044] = 1'b1;    9'b100010100: dec128[028] = 1'b1;    9'b110001000: dec128[012] = 1'b1;
   9'b100010010: dec128[043] = 1'b1;    9'b010010100: dec128[027] = 1'b1;    9'b101001000: dec128[011] = 1'b1;
   9'b010010010: dec128[042] = 1'b1;    9'b001010100: dec128[026] = 1'b1;    9'b011001000: dec128[010] = 1'b1;
   9'b000110010: dec128[041] = 1'b1;    9'b001100100: dec128[025] = 1'b1;    9'b001110000: dec128[009] = 1'b1;
   9'b110000010: dec128[040] = 1'b1;    9'b010100100: dec128[024] = 1'b1;    9'b100110000: dec128[008] = 1'b1;
   9'b101000010: dec128[039] = 1'b1;    9'b100100100: dec128[023] = 1'b1;    9'b010110000: dec128[007] = 1'b1;
   9'b011000010: dec128[038] = 1'b1;    9'b110000100: dec128[022] = 1'b1;    9'b110010000: dec128[006] = 1'b1;
   9'b001100010: dec128[037] = 1'b1;    9'b101000100: dec128[021] = 1'b1;    9'b101010000: dec128[005] = 1'b1;
   9'b010100010: dec128[036] = 1'b1;    9'b011000100: dec128[020] = 1'b1;    9'b011010000: dec128[004] = 1'b1;
   9'b100100010: dec128[035] = 1'b1;    9'b000111000: dec128[019] = 1'b1;    9'b101100000: dec128[003] = 1'b1;
   9'b000011100: dec128[034] = 1'b1;    9'b001011000: dec128[018] = 1'b1;    9'b011100000: dec128[002] = 1'b1;
   9'b000101100: dec128[033] = 1'b1;    9'b100011000: dec128[017] = 1'b1;    9'b110100000: dec128[001] = 1'b1;
   9'b100001100: dec128[032] = 1'b1;    9'b010011000: dec128[016] = 1'b1;    9'b111000000: dec128[000] = 1'b1;
   default       dec128[136:0] = 137'b0;
   endcase
end
endfunction

endmodule

// ECC GENERATE FUNCTION =======================================================================================================

module A2_ECC128_gen (
   output wire    [8:0] e,
   input  wire  [127:0] d
   );

assign      // RTT20240324 added 121 to e[0] and removed it from e[1]
   e[0] = ^{d[56], d[57], d[58], d[59], d[60], d[61], d[62], d[63], d[64], d[65], d[66], d[67], d[68], d[69], d[70], d[71],
            d[72], d[73], d[74], d[75], d[76], d[77], d[78], d[79], d[80], d[81], d[82], d[83], d[84], d[85], d[86], d[87],
            d[88], d[93], d[96], d[99], d[100],d[101],d[102],d[106],d[107],d[108],d[109],d[110],d[111],d[115],d[116],d[117],
            d[118],d[121],d[123],d[124],d[125]},

   e[1] = ^{d[35], d[36], d[37], d[38], d[39], d[40], d[41], d[42], d[43], d[44], d[45], d[46], d[47], d[48], d[49], d[50],
            d[51], d[52], d[53], d[54], d[55], d[77], d[78], d[79], d[80], d[81], d[82], d[83], d[84], d[85], d[86], d[87],
            d[88], d[92], d[95], d[99], d[100],d[101],d[102],d[105],d[107],d[108],d[109],d[110],d[111],d[114],d[119],d[120],
            d[122],d[123],d[126],d[127]},

   e[2] = ^{d[20], d[21], d[22], d[23], d[24], d[25], d[26], d[27], d[28], d[29], d[30], d[31], d[32], d[33], d[34], d[50],
            d[51], d[52], d[53], d[54], d[55], d[71], d[72], d[73], d[74], d[75], d[76], d[83], d[84], d[85], d[86], d[87],
            d[88], d[91], d[94], d[97], d[98], d[99], d[100],d[101],d[102],d[104],d[107],d[112],d[113],d[114],d[115],d[116],
            d[117],d[118],d[122],d[124],d[126]},

   e[3] = ^{d[10], d[11], d[12], d[13], d[14], d[15], d[16], d[17], d[18], d[19], d[30], d[31], d[32], d[33], d[34], d[45],
            d[46], d[47], d[48], d[49], d[55], d[66], d[67], d[68], d[69], d[70], d[76], d[82], d[84], d[85], d[86], d[87],
            d[88], d[90], d[94], d[95], d[96], d[97], d[98], d[103],d[108],d[112],d[113],d[114],d[115],d[118],d[119],d[120],
            d[121],d[124],d[125],d[127]},

   e[4] = ^{d[4],  d[5],  d[6],  d[7],  d[8],  d[9],  d[16], d[17], d[18], d[19], d[26], d[27], d[28], d[29], d[34], d[41],
            d[42], d[43], d[44], d[49], d[54], d[62], d[63], d[64], d[65], d[70], d[75], d[81], d[84], d[89], d[94], d[95],
            d[96], d[97], d[98], d[99], d[100],d[101],d[102],d[103],d[104],d[105],d[106],d[109],d[117],d[119],d[120],d[121],
            d[122],d[123],d[126],d[127]},

   e[5] = ^{d[1],  d[2],  d[3],  d[7],  d[8],  d[9],  d[13], d[14], d[15], d[19], d[23], d[24], d[25], d[29], d[33], d[35],
            d[36], d[37], d[41], d[48], d[53], d[59], d[60], d[61], d[65], d[69], d[74], d[77], d[85], d[89], d[90], d[91],
            d[92], d[93], d[94], d[95], d[96], d[97], d[98], d[99], d[110],d[112],d[113],d[114],d[115],d[116],d[121],d[122],
            d[123],d[124],d[125],d[127]},

   e[6] = ^{d[0],  d[2],  d[3],  d[4],  d[5],  d[9],  d[10], d[11], d[15], d[18], d[20], d[21], d[25], d[26], d[30], d[37],
            d[38], d[39], d[44], d[47], d[52], d[57], d[58], d[61], d[64], d[68], d[73], d[78], d[86], d[89], d[90], d[91],
            d[92], d[93], d[94], d[95], d[96], d[100],d[103],d[104],d[105],d[106],d[111],d[112],d[113],d[114],d[115],d[116],
            d[117],d[118],d[120],d[125]},

   e[7] = ^{d[0],  d[1],  d[2],  d[4],  d[6],  d[7],  d[10], d[12], d[14], d[16], d[20], d[22], d[24], d[27], d[31], d[36],
            d[38], d[40], d[42], d[46], d[50], d[56], d[58], d[60], d[63], d[66], d[71], d[80], d[87], d[89], d[90], d[91],
            d[92], d[93], d[97], d[101],d[103],d[104],d[105],d[106],d[107],d[108],d[109],d[110],d[111],d[112],d[119],d[120],
            d[121],d[122],d[123],d[126],d[127]},

   e[8] = ^{d[0],  d[1],  d[3],  d[5],  d[6],  d[8],  d[11], d[12], d[13], d[17], d[21], d[22], d[23], d[28], d[32], d[35],
            d[39], d[40], d[43], d[45], d[51], d[56], d[57], d[59], d[62], d[67], d[72], d[79], d[88], d[89], d[90], d[91],
            d[92], d[93], d[98], d[102],d[103],d[104],d[105],d[106],d[107],d[108],d[109],d[110],d[111],d[113],d[116],d[117],
            d[118],d[119],d[124],d[125],d[126]};


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

parameter   WIDTH = 128;

integer  i,j,pop,even,odd;
reg    [8:0]  num;

reg  [127:0]  data_in;       // Input data to be ecc protected and output as ecc_data_out

wire [136:0]  ecc_data;      // Protected output data
wire [127:0]  data_out;      // Output data
wire   [8:0]  code_out;      // Output corrected code bits
wire   [8:0]  check;
wire          single;
wire          double;
wire [136:0]  ecc_data_out;
reg  [136:0]  ecc_data_in;

reg  [127:0]  data_in;
reg  [136:0]  flip;

// Generate
A2_ECC128_gen mut_g1 (
   .e(ecc_data_out[136:128]), // Output 9 bit ecc
   .d(data_in[127:0])         // Input 128 bit data
   );

assign   ecc_data_out[127:0] = data_in[127:0];

// Check and Correct

A2_ECC128_dec mut_d (
   .Do (data_out),            // Output 128 bit Output data
   .Co (code_out),            // Output 9 bit Output corrected code bits
   .sec(single),              // Output Single error corrected event
   .ded(double),              // Output Double error detected  event
   .iD (ecc_data_in[136:0])   // Input 136 bit Protected input data to be checked and corrected and output
   );

initial begin
   $display("\n\n No errors");
   data_in  =      128'h0123_4567_89ab_cdef_fedc_ba98_7654_3210;
   flip     = 137'h0_00_0000_0000_0000_0000_0000_0000_0000_0000;
   for (i=0; i<137; i=i+1) begin
      #10   ecc_data_in =  ecc_data_out ^ flip;                               // force a one-bit error
      #10   if (data_out != data_in)   $display(" Error at position %d",(136-i));
            $display("%3d: di = %h, do = %h, flip = %h, syn = %b, iD = %b, iC = %b, sec = %b ded = %b",
              (136-i),data_in, data_out, flip, mut_d.syndrome, ecc_data_in[136:128], mut_d.check, single, double);
      #10   flip        =  {flip[0],flip[136:1]};
            data_in     =  {data_in[0],data_in[127:1]};
      #10;
      end
   // Single bit error correction
   $display(" Single error correction test");
   data_in  =      128'h0000_0000_0000_0000_0000_0000_0000_0000;
   flip     = 137'h1_00_0000_0000_0000_0000_0000_0000_0000_0000;
   for (i=0; i<137; i=i+1) begin
      #10   ecc_data_in =  ecc_data_out ^ flip;                               // force a one-bit error
      #10   if (data_out != data_in)   $display(" Error at position %d",(136-i));
            $display("%3d: di = %h, do = %h, flip = %h, syn = %b, iD = %b, iC = %b, sec = %b ded = %b",
              (136-i),data_in, data_out, flip, mut_d.syndrome, ecc_data_in[136:128], mut_d.check, single, double);
      #10   flip        =  {flip[0],flip[136:1]};
      #10;
      end
   // Double bit error detection
   $display(" Double error detection test");
   data_in  = 128'h8000_0000_0000_0000_8000_0000_0000_0000;
   flip     = 128'h6000_0000_0000_0000_0000_0000_0000_0000;
   for (i=0; i<128; i=i+1) begin
      #10   ecc_data_in =  ecc_data_out ^ {9'h0,flip};
      #10   $write("%3d: di = %h, do = %h, flip = %h, syn = %b, iD = %b, iC = %b, sec = %b ded = %b",
              (136-i),data_in, data_out, flip, mut_d.syndrome, ecc_data_in[136:128], mut_d.check, single, double);
            if (data_out != data_in)   $display(" Error at position %d",(136-i));
            else                       $display(" ");
      #10   data_in     = {data_in[127],data_in[127:1]};
            flip        = {   flip[127],   flip[127:1]};
      #10;
      end
   // Multi bit error
   $display(" Multi-bit errors ");
   data_in  = 128'h8000_0000_0000_0000_8000_0000_0000_0000;
   flip     = 128'h7000_0000_0000_0000_0000_0000_0000_0000;
   for (i=0; i<128; i=i+1) begin
      #10   ecc_data_in =  ecc_data_out ^ {9'h0,flip};
      #10   $write("%3d: di = %h, do = %h, flip = %h, syn = %b, iD = %b, iC = %b, sec = %b ded = %b",
              (136-i),data_in, data_out, flip, mut_d.syndrome, ecc_data_in[136:128], mut_d.check, single, double);
            if (data_out != data_in)   $display(" Error at position %d",(136-i));
            else                       $display(" ");
      #10   data_in     = {data_in[127],data_in[127:1]};
            flip        = {   flip[127],   flip[127:1]};
      #10;
      end

`ifdef   RANDOM_TEST
   $display("\n Random testing \n ");

   for (i=0; i<1000_000_000; i=i+1) begin             // Takes a while!!! :(
      data_in  = {$random,$random,$random,$random};
      flip     =  1 <<  (i % 136);
      #10   ecc_data_in =  ecc_data_out ^ flip;
      #10   if (double || !single || i < 10) $display("%3d: di = %h, do = %h, flip = %h, syn = %b, iD = %b, iC = %b, sec = %b ded = %b",
              (136-i),data_in, data_out, flip, mut_d.syndrome, ecc_data_in[136:128], mut_d.check, single, double);
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
`ifdef   STUFF

 Odd :  000000001  has   1 bits
 Odd :  000000010  has   1 bits
 Odd :  000000100  has   1 bits
 Odd :  000001000  has   1 bits
 Odd :  000010000  has   1 bits
 Odd :  000100000  has   1 bits
 Odd :  001000000  has   1 bits
 Odd :  010000000  has   1 bits
 Odd :  100000000  has   1 bits

 Odd :  000000111  has   3 bits
 Odd :  000001011  has   3 bits
 Odd :  000001101  has   3 bits
 Odd :  000001110  has   3 bits
 Odd :  000010011  has   3 bits
 Odd :  000010101  has   3 bits
 Odd :  000010110  has   3 bits
 Odd :  000011001  has   3 bits
 Odd :  000011010  has   3 bits
 Odd :  000011100  has   3 bits

 Odd :  000100011  has   3 bits
 Odd :  000100101  has   3 bits
 Odd :  000100110  has   3 bits
 Odd :  000101001  has   3 bits
 Odd :  000101010  has   3 bits
 Odd :  000101100  has   3 bits
 Odd :  000110001  has   3 bits
 Odd :  000110010  has   3 bits
 Odd :  000110100  has   3 bits
 Odd :  000111000  has   3 bits

 Odd :  001000011  has   3 bits
 Odd :  001000101  has   3 bits
 Odd :  001000110  has   3 bits
 Odd :  001001001  has   3 bits
 Odd :  001001010  has   3 bits
 Odd :  001001100  has   3 bits
 Odd :  001010001  has   3 bits
 Odd :  001010010  has   3 bits
 Odd :  001010100  has   3 bits
 Odd :  001011000  has   3 bits
 Odd :  001100001  has   3 bits
 Odd :  001100010  has   3 bits
 Odd :  001100100  has   3 bits
 Odd :  001101000  has   3 bits
 Odd :  001110000  has   3 bits

 Odd :  010000011  has   3 bits
 Odd :  010000101  has   3 bits
 Odd :  010000110  has   3 bits
 Odd :  010001001  has   3 bits
 Odd :  010001010  has   3 bits
 Odd :  010001100  has   3 bits
 Odd :  010010001  has   3 bits
 Odd :  010010010  has   3 bits
 Odd :  010010100  has   3 bits
 Odd :  010011000  has   3 bits
 Odd :  010100001  has   3 bits
 Odd :  010100010  has   3 bits
 Odd :  010100100  has   3 bits
 Odd :  010101000  has   3 bits
 Odd :  010110000  has   3 bits
 Odd :  011000001  has   3 bits
 Odd :  011000010  has   3 bits
 Odd :  011000100  has   3 bits
 Odd :  011001000  has   3 bits
 Odd :  011010000  has   3 bits
 Odd :  011100000  has   3 bits

 Odd :  100000011  has   3 bits
 Odd :  100000101  has   3 bits
 Odd :  100000110  has   3 bits
 Odd :  100001001  has   3 bits
 Odd :  100001010  has   3 bits
 Odd :  100001100  has   3 bits
 Odd :  100010001  has   3 bits
 Odd :  100010010  has   3 bits
 Odd :  100010100  has   3 bits
 Odd :  100011000  has   3 bits
 Odd :  100100001  has   3 bits
 Odd :  100100010  has   3 bits
 Odd :  100100100  has   3 bits
 Odd :  100101000  has   3 bits
 Odd :  100110000  has   3 bits
 Odd :  101000001  has   3 bits
 Odd :  101000010  has   3 bits
 Odd :  101000100  has   3 bits
 Odd :  101001000  has   3 bits
 Odd :  101010000  has   3 bits
 Odd :  101100000  has   3 bits
 Odd :  110000001  has   3 bits
 Odd :  110000010  has   3 bits
 Odd :  110000100  has   3 bits
 Odd :  110001000  has   3 bits
 Odd :  110010000  has   3 bits
 Odd :  110100000  has   3 bits
 Odd :  111000000  has   3 bits

 Odd :  000011111  has   5 bits
 Odd :  000101111  has   5 bits
 Odd :  000110111  has   5 bits
 Odd :  000111011  has   5 bits
 Odd :  000111101  has   5 bits
 Odd :  000111110  has   5 bits
 Odd :  001001111  has   5 bits
 Odd :  001010111  has   5 bits
 Odd :  001011011  has   5 bits
 Odd :  001011101  has   5 bits
 Odd :  001011110  has   5 bits
 Odd :  001100111  has   5 bits
 Odd :  001101011  has   5 bits
 Odd :  001101101  has   5 bits
 Odd :  001101110  has   5 bits
 Odd :  001110011  has   5 bits
 Odd :  001110101  has   5 bits
 Odd :  001110110  has   5 bits
 Odd :  001111001  has   5 bits
 Odd :  001111010  has   5 bits
 Odd :  001111100  has   5 bits
 Odd :  010001111  has   5 bits
 Odd :  010010111  has   5 bits
 Odd :  010011011  has   5 bits
 Odd :  010011101  has   5 bits
 Odd :  010011110  has   5 bits
 Odd :  010100111  has   5 bits
 Odd :  010101011  has   5 bits
 Odd :  010101101  has   5 bits
 Odd :  010101110  has   5 bits
 Odd :  010110011  has   5 bits
 Odd :  010110101  has   5 bits
 Odd :  010110110  has   5 bits
 Odd :  010111001  has   5 bits
 Odd :  010111010  has   5 bits
 Odd :  010111100  has   5 bits
 Odd :  011000111  has   5 bits
 Odd :  011001011  has   5 bits
 Odd :  011001101  has   5 bits
 Odd :  011001110  has   5 bits
 Odd :  011010011  has   5 bits
 Odd :  011010101  has   5 bits
 Odd :  011010110  has   5 bits
 Odd :  011011001  has   5 bits
 Odd :  011011010  has   5 bits
 Odd :  011011100  has   5 bits
 Odd :  011100011  has   5 bits
 Odd :  011100101  has   5 bits
 Odd :  011100110  has   5 bits
 Odd :  011101001  has   5 bits
 Odd :  011101010  has   5 bits
 Odd :  011101100  has   5 bits
 Odd :  011110001  has   5 bits
 Odd :  011110010  has   5 bits
 Odd :  011110100  has   5 bits
 Odd :  011111000  has   5 bits
 Odd :  100001111  has   5 bits
 Odd :  100010111  has   5 bits
 Odd :  100011011  has   5 bits
 Odd :  100011101  has   5 bits
 Odd :  100011110  has   5 bits
 Odd :  100100111  has   5 bits
 Odd :  100101011  has   5 bits
 Odd :  100101101  has   5 bits
 Odd :  100101110  has   5 bits
 Odd :  100110011  has   5 bits
 Odd :  100110101  has   5 bits
 Odd :  100110110  has   5 bits
 Odd :  100111001  has   5 bits
 Odd :  100111010  has   5 bits
 Odd :  100111100  has   5 bits
 Odd :  101000111  has   5 bits
 Odd :  101001011  has   5 bits
 Odd :  101001101  has   5 bits
 Odd :  101001110  has   5 bits
 Odd :  101010011  has   5 bits
 Odd :  101010101  has   5 bits
 Odd :  101010110  has   5 bits
 Odd :  101011001  has   5 bits
 Odd :  101011010  has   5 bits
 Odd :  101011100  has   5 bits
 Odd :  101100011  has   5 bits
 Odd :  101100101  has   5 bits
 Odd :  101100110  has   5 bits
 Odd :  101101001  has   5 bits
 Odd :  101101010  has   5 bits
 Odd :  101101100  has   5 bits
 Odd :  101110001  has   5 bits
 Odd :  101110010  has   5 bits
 Odd :  101110100  has   5 bits
 Odd :  101111000  has   5 bits
 Odd :  110000111  has   5 bits
 Odd :  110001011  has   5 bits
 Odd :  110001101  has   5 bits
 Odd :  110001110  has   5 bits
 Odd :  110010011  has   5 bits
 Odd :  110010101  has   5 bits
 Odd :  110010110  has   5 bits
 Odd :  110011001  has   5 bits
 Odd :  110011010  has   5 bits
 Odd :  110011100  has   5 bits
 Odd :  110100011  has   5 bits
 Odd :  110100101  has   5 bits
 Odd :  110100110  has   5 bits
 Odd :  110101001  has   5 bits
 Odd :  110101010  has   5 bits
 Odd :  110101100  has   5 bits
 Odd :  110110001  has   5 bits
 Odd :  110110010  has   5 bits
 Odd :  110110100  has   5 bits
 Odd :  110111000  has   5 bits
 Odd :  111000011  has   5 bits
 Odd :  111000101  has   5 bits
 Odd :  111000110  has   5 bits
 Odd :  111001001  has   5 bits
 Odd :  111001010  has   5 bits
 Odd :  111001100  has   5 bits
 Odd :  111010001  has   5 bits
 Odd :  111010010  has   5 bits
 Odd :  111010100  has   5 bits
 Odd :  111011000  has   5 bits
 Odd :  111100001  has   5 bits
 Odd :  111100010  has   5 bits
 Odd :  111100100  has   5 bits
 Odd :  111101000  has   5 bits
 Odd :  111110000  has   5 bits

 Odd :  001111111  has   7 bits
 Odd :  010111111  has   7 bits
 Odd :  011011111  has   7 bits
 Odd :  011101111  has   7 bits
 Odd :  011110111  has   7 bits
 Odd :  011111011  has   7 bits
 Odd :  011111101  has   7 bits
 Odd :  011111110  has   7 bits
 Odd :  100111111  has   7 bits
 Odd :  101011111  has   7 bits
 Odd :  101101111  has   7 bits
 Odd :  101110111  has   7 bits
 Odd :  101111011  has   7 bits
 Odd :  101111101  has   7 bits
 Odd :  101111110  has   7 bits
 Odd :  110011111  has   7 bits
 Odd :  110101111  has   7 bits
 Odd :  110110111  has   7 bits
 Odd :  110111011  has   7 bits
 Odd :  110111101  has   7 bits
 Odd :  110111110  has   7 bits
 Odd :  111001111  has   7 bits
 Odd :  111010111  has   7 bits
 Odd :  111011011  has   7 bits
 Odd :  111011101  has   7 bits
 Odd :  111011110  has   7 bits
 Odd :  111100111  has   7 bits
 Odd :  111101011  has   7 bits
 Odd :  111101101  has   7 bits
 Odd :  111101110  has   7 bits
 Odd :  111110011  has   7 bits
 Odd :  111110101  has   7 bits
 Odd :  111110110  has   7 bits
 Odd :  111111001  has   7 bits
 Odd :  111111010  has   7 bits
 Odd :  111111100  has   7 bits
 Odd :  111111111  has   9 bits

// syndromes
9'b000000001: dec128[128] = 1'b1;
9'b000000010: dec128[129] = 1'b1;
9'b000000100: dec128[130] = 1'b1;
9'b000001000: dec128[131] = 1'b1;
9'b000010000: dec128[132] = 1'b1;
9'b000100000: dec128[133] = 1'b1;
9'b001000000: dec128[134] = 1'b1;
9'b010000000: dec128[135] = 1'b1;
9'b100000000: dec128[136] = 1'b1;

9'b000000111: dec128[083] = 1'b1;    Odd :  000000111  has   3 bits
9'b000001011: dec128[082] = 1'b1;    Odd :  000001011  has   3 bits
9'b000001101: dec128[076] = 1'b1;    Odd :  000001101  has   3 bits
9'b000001110: dec128[055] = 1'b1;    Odd :  000001110  has   3 bits
9'b000010011: dec128[081] = 1'b1;    Odd :  000010011  has   3 bits
9'b000010101: dec128[075] = 1'b1;    Odd :  000010101  has   3 bits
9'b000010110: dec128[054] = 1'b1;    Odd :  000010110  has   3 bits
9'b000011001: dec128[070] = 1'b1;    Odd :  000011001  has   3 bits
9'b000011010: dec128[049] = 1'b1;    Odd :  000011010  has   3 bits
9'b000011100: dec128[034] = 1'b1;    Odd :  000011100  has   3 bits
9'b000100011: dec128[077] = 1'b1;    Odd :  000100011  has   3 bits
9'b000100101: dec128[074] = 1'b1;    Odd :  000100101  has   3 bits
9'b000100110: dec128[053] = 1'b1;    Odd :  000100110  has   3 bits
9'b000101001: dec128[069] = 1'b1;    Odd :  000101001  has   3 bits
9'b000101010: dec128[048] = 1'b1;    Odd :  000101010  has   3 bits
9'b000101100: dec128[033] = 1'b1;    Odd :  000101100  has   3 bits
9'b000110001: dec128[065] = 1'b1;    Odd :  000110001  has   3 bits
9'b000110010: dec128[041] = 1'b1;    Odd :  000110010  has   3 bits
9'b000110100: dec128[029] = 1'b1;    Odd :  000110100  has   3 bits
9'b000111000: dec128[019] = 1'b1;    Odd :  000111000  has   3 bits
9'b001000011: dec128[078] = 1'b1;    Odd :  001000011  has   3 bits
9'b001000101: dec128[073] = 1'b1;    Odd :  001000101  has   3 bits
9'b001000110: dec128[052] = 1'b1;    Odd :  001000110  has   3 bits
9'b001001001: dec128[068] = 1'b1;    Odd :  001001001  has   3 bits
9'b001001010: dec128[047] = 1'b1;    Odd :  001001010  has   3 bits
9'b001001100: dec128[030] = 1'b1;    Odd :  001001100  has   3 bits
9'b001010001: dec128[064] = 1'b1;    Odd :  001010001  has   3 bits
9'b001010010: dec128[044] = 1'b1;    Odd :  001010010  has   3 bits
9'b001010100: dec128[026] = 1'b1;    Odd :  001010100  has   3 bits
9'b001011000: dec128[018] = 1'b1;    Odd :  001011000  has   3 bits
9'b001100001: dec128[061] = 1'b1;    Odd :  001100001  has   3 bits
9'b001100010: dec128[037] = 1'b1;    Odd :  001100010  has   3 bits
9'b001100100: dec128[025] = 1'b1;    Odd :  001100100  has   3 bits
9'b001101000: dec128[015] = 1'b1;    Odd :  001101000  has   3 bits
9'b001110000: dec128[009] = 1'b1;    Odd :  001110000  has   3 bits
9'b010000011: dec128[080] = 1'b1;    Odd :  010000011  has   3 bits
9'b010000101: dec128[071] = 1'b1;    Odd :  010000101  has   3 bits
9'b010000110: dec128[050] = 1'b1;    Odd :  010000110  has   3 bits
9'b010001001: dec128[066] = 1'b1;    Odd :  010001001  has   3 bits
9'b010001010: dec128[046] = 1'b1;    Odd :  010001010  has   3 bits
9'b010001100: dec128[031] = 1'b1;    Odd :  010001100  has   3 bits
9'b010010001: dec128[063] = 1'b1;    Odd :  010010001  has   3 bits
9'b010010010: dec128[042] = 1'b1;    Odd :  010010010  has   3 bits
9'b010010100: dec128[027] = 1'b1;    Odd :  010010100  has   3 bits
9'b010011000: dec128[016] = 1'b1;    Odd :  010011000  has   3 bits
9'b010100001: dec128[060] = 1'b1;    Odd :  010100001  has   3 bits
9'b010100010: dec128[036] = 1'b1;    Odd :  010100010  has   3 bits
9'b010100100: dec128[024] = 1'b1;    Odd :  010100100  has   3 bits
9'b010101000: dec128[014] = 1'b1;    Odd :  010101000  has   3 bits
9'b010110000: dec128[007] = 1'b1;    Odd :  010110000  has   3 bits
9'b011000001: dec128[058] = 1'b1;    Odd :  011000001  has   3 bits
9'b011000010: dec128[038] = 1'b1;    Odd :  011000010  has   3 bits
9'b011000100: dec128[020] = 1'b1;    Odd :  011000100  has   3 bits
9'b011001000: dec128[010] = 1'b1;    Odd :  011001000  has   3 bits
9'b011010000: dec128[004] = 1'b1;    Odd :  011010000  has   3 bits
9'b011100000: dec128[002] = 1'b1;    Odd :  011100000  has   3 bits
9'b100000011: dec128[079] = 1'b1;    Odd :  100000011  has   3 bits
9'b100000101: dec128[072] = 1'b1;    Odd :  100000101  has   3 bits
9'b100000110: dec128[051] = 1'b1;    Odd :  100000110  has   3 bits
9'b100001001: dec128[067] = 1'b1;    Odd :  100001001  has   3 bits
9'b100001010: dec128[045] = 1'b1;    Odd :  100001010  has   3 bits
9'b100001100: dec128[032] = 1'b1;    Odd :  100001100  has   3 bits
9'b100010001: dec128[062] = 1'b1;    Odd :  100010001  has   3 bits
9'b100010010: dec128[043] = 1'b1;    Odd :  100010010  has   3 bits
9'b100010100: dec128[028] = 1'b1;    Odd :  100010100  has   3 bits
9'b100011000: dec128[017] = 1'b1;    Odd :  100011000  has   3 bits
9'b100100001: dec128[059] = 1'b1;    Odd :  100100001  has   3 bits
9'b100100010: dec128[035] = 1'b1;    Odd :  100100010  has   3 bits
9'b100100100: dec128[023] = 1'b1;    Odd :  100100100  has   3 bits
9'b100101000: dec128[013] = 1'b1;    Odd :  100101000  has   3 bits
9'b100110000: dec128[008] = 1'b1;    Odd :  100110000  has   3 bits
9'b101000001: dec128[057] = 1'b1;    Odd :  101000001  has   3 bits
9'b101000010: dec128[039] = 1'b1;    Odd :  101000010  has   3 bits
9'b101000100: dec128[021] = 1'b1;    Odd :  101000100  has   3 bits
9'b101001000: dec128[011] = 1'b1;    Odd :  101001000  has   3 bits
9'b101010000: dec128[005] = 1'b1;    Odd :  101010000  has   3 bits
9'b101100000: dec128[003] = 1'b1;    Odd :  101100000  has   3 bits
9'b110000001: dec128[056] = 1'b1;    Odd :  110000001  has   3 bits
9'b110000010: dec128[040] = 1'b1;    Odd :  110000010  has   3 bits
9'b110000100: dec128[022] = 1'b1;    Odd :  110000100  has   3 bits
9'b110001000: dec128[012] = 1'b1;    Odd :  110001000  has   3 bits
9'b110010000: dec128[006] = 1'b1;    Odd :  110010000  has   3 bits
9'b110100000: dec128[001] = 1'b1;    Odd :  110100000  has   3 bits
9'b111000000: dec128[000] = 1'b1;    Odd :  111000000  has   3 bits

9'b010111100: dec128[097] = 1'b1;
9'b011011010: dec128[120] = 1'b1;
9'b011101100: dec128[112] = 1'b1;
9'b100001111: dec128[088] = 1'b1;
9'b100010111: dec128[102] = 1'b1;
9'b100101101: dec128[124] = 1'b1;
9'b100111100: dec128[098] = 1'b1;
9'b101001101: dec128[118] = 1'b1;
9'b101010101: dec128[117] = 1'b1;
9'b101100101: dec128[116] = 1'b1;
9'b101101001: dec128[125] = 1'b1;
9'b101101100: dec128[113] = 1'b1;
9'b110000111: dec128[107] = 1'b1;
9'b110001011: dec128[108] = 1'b1;
9'b110010011: dec128[109] = 1'b1;
9'b110010110: dec128[126] = 1'b1;
9'b110011010: dec128[119] = 1'b1;
9'b110100011: dec128[110] = 1'b1;
9'b111000011: dec128[111] = 1'b1;
9'b111010001: dec128[106] = 1'b1;
9'b111010010: dec128[105] = 1'b1;
9'b111010100: dec128[104] = 1'b1;
9'b111011000: dec128[103] = 1'b1;
9'b111100001: dec128[093] = 1'b1;
9'b111100010: dec128[092] = 1'b1;
9'b111100100: dec128[091] = 1'b1;
9'b111101000: dec128[090] = 1'b1;
9'b111110000: dec128[089] = 1'b1;

9'b000011111: dec128[084] = 1'b1;    Odd :  000011111  has   5 bits
                                     Odd :  000101111  has   5 bits
9'b000110111: dec128[099] = 1'b1;    Odd :  000110111  has   5 bits
                                     Odd :  000111011  has   5 bits
                                     Odd :  000111101  has   5 bits
                                     Odd :  000111110  has   5 bits
9'b001001111: dec128[086] = 1'b1;    Odd :  001001111  has   5 bits
9'b001010111: dec128[100] = 1'b1;    Odd :  001010111  has   5 bits
                                     Odd :  001011011  has   5 bits
                                     Odd :  001011101  has   5 bits
                                     Odd :  001011110  has   5 bits
                                     Odd :  001100111  has   5 bits
                                     Odd :  001101011  has   5 bits
9'b001101101: dec128[115] = 1'b1;    Odd :  001101101  has   5 bits
9'b001101110: dec128[114] = 1'b1;    Odd :  001101110  has   5 bits
                                     Odd :  001110011  has   5 bits
                                     Odd :  001110101  has   5 bits
                                     Odd :  001110110  has   5 bits
9'b001111001: dec128[096] = 1'b1;    Odd :  001111001  has   5 bits
9'b001111010: dec128[095] = 1'b1;    Odd :  001111010  has   5 bits
9'b001111100: dec128[094] = 1'b1;    Odd :  001111100  has   5 bits
9'b010001111: dec128[087] = 1'b1;    Odd :  010001111  has   5 bits
9'b010010111: dec128[101] = 1'b1;    Odd :  010010111  has   5 bits
9'b010110011: dec128[123] = 1'b1;    Odd :  010011011  has   5 bits
                                     Odd :  010011101  has   5 bits
                                     Odd :  010011110  has   5 bits
                                     Odd :  010100111  has   5 bits
                                     Odd :  010101011  has   5 bits
                                     Odd :  010101101  has   5 bits
                                     Odd :  010101110  has   5 bits
                                     Odd :  010110011  has   5 bits
                                     Odd :  010110101  has   5 bits
9'b010110110: dec128[122] = 1'b1;    Odd :  010110110  has   5 bits
9'b010111001: dec128[121] = 1'b1;    Odd :  010111001  has   5 bits
9'b010111010: dec128[127] = 1'b1;    Odd :  010111010  has   5 bits
                                     Odd :  010111100  has   5 bits
                                     Odd :  011000111  has   5 bits
                                     Odd :  011001011  has   5 bits
                                     Odd :  011001101  has   5 bits
                                     Odd :  011001110  has   5 bits
                                     Odd :  011010011  has   5 bits
                                     Odd :  011010101  has   5 bits
                                     Odd :  011010110  has   5 bits
                                     Odd :  011011001  has   5 bits
                                     Odd :  011011010  has   5 bits
                                     Odd :  011011100  has   5 bits
                                     Odd :  011100011  has   5 bits
                                     Odd :  011100101  has   5 bits
                                     Odd :  011100110  has   5 bits
                                     Odd :  011101001  has   5 bits
                                     Odd :  011101010  has   5 bits
                                     Odd :  011101100  has   5 bits
                                     Odd :  011110001  has   5 bits
                                     Odd :  011110010  has   5 bits
                                     Odd :  011110100  has   5 bits
                                     Odd :  011111000  has   5 bits
                                     Odd :  100001111  has   5 bits
                                     Odd :  100010111  has   5 bits
                                     Odd :  100011011  has   5 bits
                                     Odd :  100011101  has   5 bits
                                     Odd :  100011110  has   5 bits
                                     Odd :  100100111  has   5 bits
                                     Odd :  100101011  has   5 bits
                                     Odd :  100101101  has   5 bits
                                     Odd :  100101110  has   5 bits
                                     Odd :  100110011  has   5 bits
                                     Odd :  100110101  has   5 bits
                                     Odd :  100110110  has   5 bits
                                     Odd :  100111001  has   5 bits
                                     Odd :  100111010  has   5 bits
                                     Odd :  100111100  has   5 bits
                                     Odd :  101000111  has   5 bits
                                     Odd :  101001011  has   5 bits
                                     Odd :  101001101  has   5 bits
                                     Odd :  101001110  has   5 bits
                                     Odd :  101010011  has   5 bits
                                     Odd :  101010101  has   5 bits
                                     Odd :  101010110  has   5 bits
                                     Odd :  101011001  has   5 bits
                                     Odd :  101011010  has   5 bits
                                     Odd :  101011100  has   5 bits
                                     Odd :  101100011  has   5 bits
                                     Odd :  101100101  has   5 bits
                                     Odd :  101100110  has   5 bits
                                     Odd :  101101001  has   5 bits
                                     Odd :  101101010  has   5 bits
                                     Odd :  101101100  has   5 bits
                                     Odd :  101110001  has   5 bits
                                     Odd :  101110010  has   5 bits
                                     Odd :  101110100  has   5 bits
                                     Odd :  101111000  has   5 bits
                                     Odd :  110000111  has   5 bits
                                     Odd :  110001011  has   5 bits
                                     Odd :  110001101  has   5 bits
                                     Odd :  110001110  has   5 bits
                                     Odd :  110010011  has   5 bits
                                     Odd :  110010101  has   5 bits
                                     Odd :  110010110  has   5 bits
                                     Odd :  110011001  has   5 bits
                                     Odd :  110011010  has   5 bits
                                     Odd :  110011100  has   5 bits
                                     Odd :  110100011  has   5 bits
                                     Odd :  110100101  has   5 bits
                                     Odd :  110100110  has   5 bits
                                     Odd :  110101001  has   5 bits
                                     Odd :  110101010  has   5 bits
                                     Odd :  110101100  has   5 bits
                                     Odd :  110110001  has   5 bits
                                     Odd :  110110010  has   5 bits
                                     Odd :  110110100  has   5 bits
                                     Odd :  110111000  has   5 bits
                                     Odd :  111000011  has   5 bits
                                     Odd :  111000101  has   5 bits
                                     Odd :  111000110  has   5 bits
                                     Odd :  111001001  has   5 bits
                                     Odd :  111001010  has   5 bits
                                     Odd :  111001100  has   5 bits
                                     Odd :  111010001  has   5 bits
                                     Odd :  111010010  has   5 bits
                                     Odd :  111010100  has   5 bits
                                     Odd :  111011000  has   5 bits
                                     Odd :  111100001  has   5 bits
                                     Odd :  111100010  has   5 bits
                                     Odd :  111100100  has   5 bits
                                     Odd :  111101000  has   5 bits
                                     Odd :  111110000  has   5 bits

`endif
