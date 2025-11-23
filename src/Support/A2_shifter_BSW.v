//================================================================================================================================
//
//   Copyright (c) 2000 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name         :  CPU Function Units
//
//   Description          :  Byte/Short/Word Shifter
//
//================================================================================================================================
//
//   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
//================================================================================================================================
//
//
//================================================================================================================================

module   A2_shifter_BSW (
   output wire [31:0]    SHo,       // Shift Data Out
   input  wire [31:0]   iSH,        // Shift Data In
   input  wire [ 4:0]   dSH,        // Shift distance
   input  wire [ 1:0]   tSH,        // Shift type
   input  wire [ 1:0]   zSH         // Shift size
   );

localparam  [1:0]  Byte  = 0, Shrt  = 1, Word = 2;
localparam  [1:0]  Right = 0, Arith = 1, Left = 2;

wire           byt,  shrt,  wrd, right, arith, left;
wire  [31:00]  word_mask_left, word_mask_right, mask_left, mask_right, mask, fill;
wire  [15:00]  shrt_mask_left, shrt_mask_right;
wire  [07:00]  byte_mask_left, byte_mask_right, byte3sign, byte2sign, byte1sign, byte0sign;
wire  [04:00]  shift, distance;

assign
   {wrd, shrt, byt} =  1'b1 << zSH[1:0],
   {left,arith,right} =  1'b1 << tSH[1:0];

assign
   shift             = byt  ? {2'b00,dSH[2:0]}
                     : shrt  ? {1'b0, dSH[3:0]}
                     :                dSH[4:0],
   distance          = ({5{left}} ^ shift ) + left;

wire  [31:0]
   rot0              =  distance[0] ? {    iSH[0], iSH[31: 1]} :  iSH[31:0],
   rot1              =  distance[1] ? {rot0[ 1:0],rot0[31: 2]} : rot0[31:0],
   rot2              =  distance[2] ? {rot1[ 3:0],rot1[31: 4]} : rot1[31:0],
   rot3              =  distance[3] ? {rot2[ 7:0],rot2[31: 8]} : rot2[31:0],
   rotate            =  distance[4] ? {rot3[15:0],rot3[31:16]} : rot3[31:0];

assign
   byte_mask_left    =  8'hff << dSH[2:0],
   byte_mask_right   =  8'hff >> dSH[2:0];

assign
   shrt_mask_left    =  {8'hff,     byte_mask_left[7:0]} << (8* dSH[3]),
   shrt_mask_right   =  {byte_mask_right[7:0],    8'hff} >> (8* dSH[3]);

assign
   word_mask_left    =  {16'hffff, shrt_mask_left[15:0]} << (16* dSH[4]),
   word_mask_right   =  {shrt_mask_right[15:0],16'hffff} >> (16* dSH[4]),

   mask_left         =  byt  ? {4{byte_mask_left [07:0]}} :  shrt  ? {2{shrt_mask_left [15:0]}} :   word_mask_left [31:0],
   mask_right        =  byt  ? {4{byte_mask_right[07:0]}} :  shrt  ? {2{shrt_mask_right[15:0]}} :   word_mask_right[31:0],

   mask              =  left  ?    mask_left : mask_right;

assign
   byte3sign         =  {8{iSH[31]}},
   byte2sign         =  {8{iSH[23]}},
   byte1sign         =  {8{iSH[15]}},
   byte0sign         =  {8{iSH[07]}};

assign
   fill[31:24]       =  arith          ? byte3sign :  8'h00,
   fill[23:16]       =  arith && !byt ? byte3sign
                     :  arith &&  byt ? byte2sign :  8'h00,
   fill[15:08]       =  arith &&  wrd ? byte3sign
                     :  arith && !wrd ? byte1sign :  8'h00,
   fill[07:00]       =  arith &&  wrd ? byte3sign
                     :  arith &&  shrt ? byte1sign
                     :  arith &&  byt ? byte0sign :  8'h00;

assign
   SHo[31:0]         =  mask & rotate | ~mask & fill;

endmodule

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////


`ifdef   Verify_A2_shifter_BSW

module t;

integer  cn, errors;
localparam  [1:0]  Byte  = 0, Shrt  = 1, Word = 2;
localparam  [1:0]  Right = 0, Arith = 1, Left = 2;

reg   [31:0]   iSH;        // Shift Data In
reg   [ 4:0]   dSH;        // Shift distance
reg   [ 1:0]   tSH;        // Shift type
reg   [ 1:0]   zSH;        // Shift size
wire  [31:0]    SHo;

A2_shifter_BSW mut (
    .SHo (SHo),         // Shift Data Out
   .iSH  (iSH),         // Shift Data In
   .dSH  (dSH),         // Shift distance
   .zSH  (zSH),         // Shift size
   .tSH  (tSH)          // Shift type
   );

initial begin
   errors = 0;
   #100;
   cn = 000; testcase_vsv(Byte,Left,   32'h80c00001, 5'h01, 32'h00800002);
   cn = 001; testcase_vsv(Byte,Left,   32'haabbccdd, 5'h04, 32'ha0b0c0d0);
   cn = 002; testcase_vsv(Byte,Left,   32'h00010203, 5'h08, 32'h00010203);
   cn = 003; testcase_vsv(Byte,Left,   32'h00010203, 5'h20, 32'h00010203);
   cn = 004; testcase_vsv(Byte,Left,   32'h00010203, 5'h80, 32'h00010203);
   cn = 005; testcase_vsv(Byte,Left,   32'h00010203, 5'h00, 32'h00010203);
   cn = 006; testcase_vsv(Byte,Left,   32'h80c00001, 5'h09, 32'h00800002);
   cn = 007; testcase_vsv(Byte,Left,   32'h00010203, 5'h09, 32'h00020406);
   cn = 008; testcase_vsv(Byte,Left,   32'h00010203, 5'h11, 32'h00020406);
   cn = 009; testcase_vsv(Byte,Left,   32'h00010203, 5'h21, 32'h00020406);
   cn = 010; testcase_vsv(Byte,Left,   32'h00010203, 5'h81, 32'h00020406);
   cn = 011; testcase_vsv(Byte,Left,   32'h00010203, 5'h01, 32'h00020406);
   cn = 012; testcase_vsv(Byte,Left,   32'h00010203, 5'h01, 32'h00020406);
   cn = 013; testcase_vsv(Byte,Left,   32'h00010203, 5'h02, 32'h0004080c);

   cn = 014; testcase_vsv(Byte,Right,  32'h80c00001, 5'h01, 32'h40600000);
   cn = 015; testcase_vsv(Byte,Right,  32'haabbccdd, 5'h04, 32'h0a0b0c0d);
   cn = 016; testcase_vsv(Byte,Right,  32'h00010203, 5'h08, 32'h00010203);
   cn = 017; testcase_vsv(Byte,Right,  32'h00010203, 5'h20, 32'h00010203);
   cn = 018; testcase_vsv(Byte,Right,  32'h00010203, 5'h80, 32'h00010203);
   cn = 019; testcase_vsv(Byte,Right,  32'h00010203, 5'h00, 32'h00010203);
   cn = 020; testcase_vsv(Byte,Right,  32'h80c00001, 5'h09, 32'h40600000);

   cn = 021; testcase_vsv(Byte,Right,  32'h00010203, 5'h09, 32'h00000101);
   cn = 022; testcase_vsv(Byte,Right,  32'h00010203, 5'h11, 32'h00000101);
   cn = 023; testcase_vsv(Byte,Right,  32'h00010203, 5'h21, 32'h00000101);
   cn = 024; testcase_vsv(Byte,Right,  32'h00010203, 5'h81, 32'h00000101);
   cn = 025; testcase_vsv(Byte,Right,  32'h00010203, 5'h01, 32'h00000101);
   cn = 026; testcase_vsv(Byte,Right,  32'h00010203, 5'h01, 32'h00000101);
   cn = 027; testcase_vsv(Byte,Right,  32'h00010203, 5'h02, 32'h00000000);
   cn = 028; testcase_vsv(Byte,Arith,  32'h80c00001, 5'h01, 32'hc0e00000);
   cn = 029; testcase_vsv(Byte,Arith,  32'haabbccdd, 5'h04, 32'hfafbfcfd);
   cn = 030; testcase_vsv(Byte,Arith,  32'h00010203, 5'h08, 32'h00010203);
   cn = 031; testcase_vsv(Byte,Arith,  32'h00010203, 5'h20, 32'h00010203);
   cn = 032; testcase_vsv(Byte,Arith,  32'h00010203, 5'h80, 32'h00010203);
   cn = 033; testcase_vsv(Byte,Arith,  32'h00010203, 5'h00, 32'h00010203);
   cn = 034; testcase_vsv(Byte,Arith,  32'h80c00001, 5'h09, 32'hc0e00000);
   cn = 035; testcase_vsv(Byte,Arith,  32'h00010203, 5'h09, 32'h00000101);
   cn = 036; testcase_vsv(Byte,Arith,  32'h00010203, 5'h11, 32'h00000101);
   cn = 037; testcase_vsv(Byte,Arith,  32'h00010203, 5'h21, 32'h00000101);
   cn = 038; testcase_vsv(Byte,Arith,  32'h00010203, 5'h81, 32'h00000101);
   cn = 039; testcase_vsv(Byte,Arith,  32'h00010203, 5'h01, 32'h00000101);
   cn = 040; testcase_vsv(Byte,Arith,  32'h00010203, 5'h01, 32'h00000101);
   cn = 041; testcase_vsv(Byte,Arith,  32'h00010203, 5'h02, 32'h00000000);
   cn = 042; testcase_vsv(Shrt,Left,   32'hff80ffc0, 5'h01, 32'hff00ff80);
   cn = 043; testcase_vsv(Shrt,Left,   32'h30393039, 5'h02, 32'hc0e4c0e4);
   cn = 044; testcase_vsv(Shrt,Left,   32'h12341234, 5'h02, 32'h48d048d0);
   cn = 045; testcase_vsv(Shrt,Left,   32'haabbccdd, 5'h04, 32'habb0cdd0);
   cn = 046; testcase_vsv(Shrt,Left,   32'h00000001, 5'h08, 32'h00000100);
   cn = 047; testcase_vsv(Shrt,Left,   32'h00000001, 5'h20, 32'h00000001);
   cn = 048; testcase_vsv(Shrt,Left,   32'h00000001, 5'h80, 32'h00000001);
   cn = 049; testcase_vsv(Shrt,Left,   32'h00000001, 5'h00, 32'h00000001);
   cn = 050; testcase_vsv(Shrt,Left,   32'hff80ffc0, 5'h11, 32'hff00ff80);
   cn = 051; testcase_vsv(Shrt,Left,   32'h00000001, 5'h11, 32'h00000002);
   cn = 052; testcase_vsv(Shrt,Left,   32'h00000001, 5'h21, 32'h00000002);
   cn = 053; testcase_vsv(Shrt,Left,   32'h00000001, 5'h81, 32'h00000002);
   cn = 054; testcase_vsv(Shrt,Left,   32'h00000001, 5'h01, 32'h00000002);
   cn = 055; testcase_vsv(Shrt,Left,   32'h00000001, 5'h01, 32'h00000002);
   cn = 056; testcase_vsv(Shrt,Left,   32'h00000001, 5'h02, 32'h00000004);
   cn = 057; testcase_vsv(Shrt,Right,  32'hff80ffc0, 5'h01, 32'h7fc07fe0);
   cn = 058; testcase_vsv(Shrt,Right,  32'h30393039, 5'h02, 32'h0c0e0c0e);
   cn = 059; testcase_vsv(Shrt,Right,  32'h90ab90ab, 5'h02, 32'h242a242a);
   cn = 060; testcase_vsv(Shrt,Right,  32'haabbccdd, 5'h04, 32'h0aab0ccd);
   cn = 061; testcase_vsv(Shrt,Right,  32'h00000001, 5'h08, 32'h00000000);
   cn = 062; testcase_vsv(Shrt,Right,  32'h00000001, 5'h20, 32'h00000001);
   cn = 063; testcase_vsv(Shrt,Right,  32'h00000001, 5'h80, 32'h00000001);
   cn = 064; testcase_vsv(Shrt,Right,  32'h00000001, 5'h00, 32'h00000001);
   cn = 065; testcase_vsv(Shrt,Right,  32'hff80ffc0, 5'h11, 32'h7fc07fe0);
   cn = 066; testcase_vsv(Shrt,Right,  32'h00000001, 5'h11, 32'h00000000);
   cn = 067; testcase_vsv(Shrt,Right,  32'h00000001, 5'h21, 32'h00000000);
   cn = 068; testcase_vsv(Shrt,Right,  32'h00000001, 5'h81, 32'h00000000);
   cn = 069; testcase_vsv(Shrt,Right,  32'h00000001, 5'h01, 32'h00000000);
   cn = 070; testcase_vsv(Shrt,Right,  32'h00000001, 5'h01, 32'h00000000);
   cn = 071; testcase_vsv(Shrt,Right,  32'h00000001, 5'h02, 32'h00000000);
   cn = 072; testcase_vsv(Shrt,Arith,  32'hff80ffc0, 5'h01, 32'hffc0ffe0);
   cn = 073; testcase_vsv(Shrt,Arith,  32'h30393039, 5'h02, 32'h0c0e0c0e);
   cn = 074; testcase_vsv(Shrt,Arith,  32'h90ab90ab, 5'h02, 32'he42ae42a);
   cn = 075; testcase_vsv(Shrt,Arith,  32'haabbccdd, 5'h04, 32'hfaabfccd);
   cn = 076; testcase_vsv(Shrt,Arith,  32'h00000001, 5'h08, 32'h00000000);
   cn = 077; testcase_vsv(Shrt,Arith,  32'h00000001, 5'h20, 32'h00000001);
   cn = 078; testcase_vsv(Shrt,Arith,  32'h00000001, 5'h80, 32'h00000001);
   cn = 079; testcase_vsv(Shrt,Arith,  32'h00000001, 5'h00, 32'h00000001);
   cn = 080; testcase_vsv(Shrt,Arith,  32'hff80ffc0, 5'h11, 32'hffc0ffe0);
   cn = 081; testcase_vsv(Shrt,Arith,  32'h00000001, 5'h11, 32'h00000000);
   cn = 082; testcase_vsv(Shrt,Arith,  32'h00000001, 5'h21, 32'h00000000);
   cn = 083; testcase_vsv(Shrt,Arith,  32'h00000001, 5'h81, 32'h00000000);
   cn = 084; testcase_vsv(Shrt,Arith,  32'h00000001, 5'h01, 32'h00000000);
   cn = 085; testcase_vsv(Shrt,Arith,  32'h00000001, 5'h01, 32'h00000000);
   cn = 086; testcase_vsv(Shrt,Arith,  32'h00000001, 5'h02, 32'h00000000);
   cn = 087; testcase_vsv(Word,Left,   32'h80000000, 5'h01, 32'h00000000);
   cn = 088; testcase_vsv(Word,Left,   32'h499602d2, 5'h02, 32'h26580b48);
   cn = 089; testcase_vsv(Word,Left,   32'h12345678, 5'h02, 32'h48d159e0);
   cn = 090; testcase_vsv(Word,Left,   32'haabbccdd, 5'h04, 32'habbccdd0);
   cn = 091; testcase_vsv(Word,Left,   32'h00000000, 5'h08, 32'h00000000);
   cn = 092; testcase_vsv(Word,Left,   32'h00000000, 5'h20, 32'h00000000);
   cn = 093; testcase_vsv(Word,Left,   32'h00000000, 5'h80, 32'h00000000);
   cn = 094; testcase_vsv(Word,Left,   32'h00000000, 5'h00, 32'h00000000);
   cn = 095; testcase_vsv(Word,Left,   32'h80000000, 5'h21, 32'h00000000);
   cn = 096; testcase_vsv(Word,Left,   32'h00000000, 5'h21, 32'h00000000);
   cn = 097; testcase_vsv(Word,Left,   32'h00000000, 5'h41, 32'h00000000);
   cn = 098; testcase_vsv(Word,Left,   32'h00000000, 5'h81, 32'h00000000);
   cn = 099; testcase_vsv(Word,Left,   32'h00000000, 5'h01, 32'h00000000);
   cn = 100; testcase_vsv(Word,Left,   32'h00000000, 5'h01, 32'h00000000);
   cn = 101; testcase_vsv(Word,Left,   32'h00000000, 5'h02, 32'h00000000);
   cn = 102; testcase_vsv(Word,Arith,  32'h80000000, 5'h01, 32'hc0000000);
   cn = 103; testcase_vsv(Word,Arith,  32'h499602d2, 5'h02, 32'h126580b4);
   cn = 104; testcase_vsv(Word,Arith,  32'h90abcdef, 5'h02, 32'he42af37b);
   cn = 105; testcase_vsv(Word,Arith,  32'haabbccdd, 5'h04, 32'hfaabbccd);
   cn = 106; testcase_vsv(Word,Arith,  32'h00000000, 5'h08, 32'h00000000);
   cn = 107; testcase_vsv(Word,Arith,  32'h00000000, 5'h20, 32'h00000000);
   cn = 108; testcase_vsv(Word,Arith,  32'h00000000, 5'h80, 32'h00000000);
   cn = 109; testcase_vsv(Word,Arith,  32'h00000000, 5'h00, 32'h00000000);
   cn = 110; testcase_vsv(Word,Arith,  32'h80000000, 5'h21, 32'hc0000000);
   cn = 111; testcase_vsv(Word,Arith,  32'h00000000, 5'h21, 32'h00000000);
   cn = 112; testcase_vsv(Word,Arith,  32'h00000000, 5'h41, 32'h00000000);
   cn = 113; testcase_vsv(Word,Arith,  32'h00000000, 5'h81, 32'h00000000);
   cn = 114; testcase_vsv(Word,Arith,  32'h00000000, 5'h01, 32'h00000000);
   cn = 115; testcase_vsv(Word,Arith,  32'h00000000, 5'h01, 32'h00000000);
   cn = 116; testcase_vsv(Word,Arith,  32'h00000000, 5'h02, 32'h00000000);
   cn = 117; testcase_vsv(Word,Arith,  32'h80000000, 5'h01, 32'hc0000000);
   cn = 118; testcase_vsv(Word,Arith,  32'h499602d2, 5'h02, 32'h126580b4);
   cn = 119; testcase_vsv(Word,Arith,  32'h90abcdef, 5'h02, 32'he42af37b);
   cn = 120; testcase_vsv(Word,Arith,  32'haabbccdd, 5'h04, 32'hfaabbccd);
   cn = 121; testcase_vsv(Word,Arith,  32'h00000000, 5'h08, 32'h00000000);
   cn = 122; testcase_vsv(Word,Arith,  32'h00000000, 5'h20, 32'h00000000);
   cn = 123; testcase_vsv(Word,Arith,  32'h00000000, 5'h80, 32'h00000000);
   cn = 124; testcase_vsv(Word,Arith,  32'h00000000, 5'h00, 32'h00000000);
   cn = 125; testcase_vsv(Word,Arith,  32'h80000000, 5'h21, 32'hc0000000);
   cn = 126; testcase_vsv(Word,Arith,  32'h00000000, 5'h21, 32'h00000000);
   cn = 127; testcase_vsv(Word,Arith,  32'h00000000, 5'h41, 32'h00000000);
   cn = 128; testcase_vsv(Word,Arith,  32'h00000000, 5'h81, 32'h00000000);
   cn = 129; testcase_vsv(Word,Arith,  32'h00000000, 5'h01, 32'h00000000);
   cn = 130; testcase_vsv(Word,Arith,  32'h00000000, 5'h01, 32'h00000000);
   cn = 131; testcase_vsv(Word,Arith,  32'h00000000, 5'h02, 32'h00000000);
   $display("\n\n  All %3d Tests Complete with %2d errors\n\n", cn, errors);
   $finish;
   end

task  testcase_vsv;   // one vector in, one scalar in, one vector out
input   [1:0]  size;
input   [1:0]  type;
input  [31:0]  data0;
input  [ 4:0]  data1;
input  [31:0]  expect;
   iSH   =  data0;
   dSH   =  data1;
   tSH   =  type;
   zSH   =  size;
   #10;
   if (SHo !== expect) begin
                       errors = errors + 1;
                       $display(" test %3d FAILED, Size=%1d, Type=%1d, Input %h by %2d (%2d) Expected %h found %h", cn, size, type, iSH, dSH, mut.distance, expect, SHo); end
   else                $display(" test %3d passed, Size=%1d, Type=%1d, Input %h by %2d (%2d) Expected %h found %h", cn, size, type, iSH, dSH, mut.distance, expect, SHo);
   #10;
endtask

endmodule

`endif
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
