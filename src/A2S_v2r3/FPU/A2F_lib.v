// **************************************************************************
// fplib: FPU RTL library
// **************************************************************************
// Copyright (c) GB3 Digital Systems, 2008. All rights reserved.
// $Id: fplib.v 1.1 2010/01/31 22:23:41 gw Exp gw $ G. Whitted
// **************************************************************************

`ifdef FP_NEGATIVE_CLOCK
   `define FP_ACTIVE_EDGE   negedge
   `define FP_INACTIVE_EDGE posedge
`else
   `define FP_POSITIVE_CLOCK
   `define FP_ACTIVE_EDGE   posedge
   `define FP_INACTIVE_EDGE negedge
`endif
`ifdef FP_tCQ
`else
   `define FP_tCQ #1
`endif
`ifdef FP_SYNC_HIGH_RESET
   `define or_FP_R_EDGE
   `define or_FP_S_EDGE
   `define FP_R_ASSERTED    r
   `define FP_S_ASSERTED    s
`else
   `ifdef FP_ASYNC_HIGH_RESET
      `define or_FP_R_EDGE  or posedge r
      `define or_FP_S_EDGE  or posedge s
      `define FP_R_ASSERTED r
      `define FP_S_ASSERTED s
   `else
      `ifdef FP_SYNC_LOW_RESET
         `define or_FP_R_EDGE
         `define or_FP_S_EDGE
         `define FP_R_ASSERTED   ~r
         `define FP_S_ASSERTED   ~s
      `else
         `define FP_ASYNC_LOW_RESET
         `define or_FP_R_EDGE  or negedge r
         `define or_FP_S_EDGE  or negedge s
         `define FP_R_ASSERTED ~r
         `define FP_S_ASSERTED ~s
      `endif
   `endif
`endif
module fp_df (q, d, clk);
parameter width = 1;
output [width-1:0] q;
input  [width-1:0] d;
input              clk;
reg [width-1:0] q;
always @(`FP_ACTIVE_EDGE clk)
   q[width-1:0] <= `FP_tCQ d[width-1:0];
endmodule
module fp_dfe (q, d, clk, e);
parameter width = 1;
output [width-1:0] q;
input  [width-1:0] d;
input              clk;
input              e;
reg    [width-1:0] q;
always @(`FP_ACTIVE_EDGE clk)
   if(e) q[width-1:0] <= `FP_tCQ d[width-1:0];
endmodule
module fp_dfr (q, d, clk, r);
parameter width = 1;
output [width-1:0] q;
input  [width-1:0] d;
input              clk;
input              r;
reg    [width-1:0] q;
always @(`FP_ACTIVE_EDGE clk `or_FP_R_EDGE)
   if(`FP_R_ASSERTED)
      q[width-1:0] <= `FP_tCQ {width{1'b0}};
   else
      q[width-1:0] <= d[width-1:0];
endmodule
module fp_dfs (q, d, clk, s);
parameter width = 1;
output [width-1:0] q;
input  [width-1:0] d;
input              clk;
input              s;
reg    [width-1:0] q;
always @(`FP_ACTIVE_EDGE clk `or_FP_S_EDGE)
   if(`FP_S_ASSERTED)
      q[width-1:0] <= `FP_tCQ {width{1'b1}};
   else
      q[width-1:0] <= d[width-1:0];
endmodule
module fp_dfer (q, d, clk, e, r);
parameter width = 1;
output [width-1:0] q;
input  [width-1:0] d;
input              clk;
input              e;
input              r;
reg    [width-1:0] q;
always @(`FP_ACTIVE_EDGE clk `or_FP_R_EDGE)
   if(`FP_R_ASSERTED)
      q[width-1:0] <= `FP_tCQ {width{1'b0}};
   else if(e)
      q[width-1:0] <= d[width-1:0];
endmodule
module fp_dfes (q, d, clk, e, s);
parameter width = 1;
output [width-1:0] q;
input  [width-1:0] d;
input              clk;
input              e;
input              s;
reg    [width-1:0] q;
always @(`FP_ACTIVE_EDGE clk `or_FP_S_EDGE)
   if(`FP_S_ASSERTED)
      q[width-1:0] <= `FP_tCQ {width{1'b1}};
   else if(e)
      q[width-1:0] <= d[width-1:0];
endmodule
module fp_mux2 (y, i0, i1, s);
parameter width = 1;
output [width-1:0] y;
input  [width-1:0] i0;
input  [width-1:0] i1;
input              s;
assign y = s ? i1 : i0;
endmodule
module fp_mux3 (y, i0, i1, i2, s);
parameter width = 1;
output [width-1:0] y;
input  [width-1:0] i0;
input  [width-1:0] i1;
input  [width-1:0] i2;
input        [1:0] s;
reg    [width-1:0] y;
always @(s or i0 or i1 or i2)
   case(s[1:0])
   0:       y = i0;
   1:       y = i1;
   2:       y = i2;
   3:       y = i2;
   default: y = {width{1'bx}};
   endcase
endmodule
module fp_mux4 (y, i0, i1, i2, i3, s);
parameter width = 1;
output [width-1:0] y;
input  [width-1:0] i0;
input  [width-1:0] i1;
input  [width-1:0] i2;
input  [width-1:0] i3;
input        [1:0] s;
reg    [width-1:0] y;
always @(s or i0 or i1 or i2 or i3)
   case(s[1:0])
   0:       y = i0;
   1:       y = i1;
   2:       y = i2;
   3:       y = i3;
   default: y = {width{1'bx}};
   endcase
endmodule
module fp_mux5 (y, i0, i1, i2, i3, i4, s);
parameter width = 1;
output [width-1:0] y;
input  [width-1:0] i0;
input  [width-1:0] i1;
input  [width-1:0] i2;
input  [width-1:0] i3;
input  [width-1:0] i4;
input        [2:0] s;
reg    [width-1:0] y;
always @(s or i0 or i1 or i2 or i3 or i4)
   case(s[2:0])
   0:       y = i0;
   1:       y = i1;
   2:       y = i2;
   3:       y = i3;
   4:       y = i4;
   5:       y = i4;
   6:       y = i4;
   7:       y = i4;
   default: y = {width{1'bx}};
   endcase
endmodule
module fp_mux6 (y, i0, i1, i2, i3, i4, i5, s);
parameter width = 1;
output [width-1:0] y;
input  [width-1:0] i0;
input  [width-1:0] i1;
input  [width-1:0] i2;
input  [width-1:0] i3;
input  [width-1:0] i4;
input  [width-1:0] i5;
input        [2:0] s;
reg    [width-1:0] y;
always @(s or i0 or i1 or i2 or i3 or i4 or i5)
   case(s[2:0])
   0:       y = i0;
   1:       y = i1;
   2:       y = i2;
   3:       y = i3;
   4:       y = i4;
   5:       y = i5;
   6:       y = i4;
   7:       y = i5;
   default: y = {width{1'bx}};
   endcase
endmodule
module fp_mux7 (y, i0, i1, i2, i3, i4, i5, i6, s);
parameter width = 1;
output [width-1:0] y;
input  [width-1:0] i0;
input  [width-1:0] i1;
input  [width-1:0] i2;
input  [width-1:0] i3;
input  [width-1:0] i4;
input  [width-1:0] i5;
input  [width-1:0] i6;
input        [2:0] s;
reg    [width-1:0] y;
always @(s or i0 or i1 or i2 or i3 or i4 or i5 or i6)
   case(s[2:0])
   0:       y = i0;
   1:       y = i1;
   2:       y = i2;
   3:       y = i3;
   4:       y = i4;
   5:       y = i5;
   6:       y = i6;
   7:       y = i6;
   default: y = {width{1'bx}};
   endcase
endmodule
module fp_mux8 (y, i0, i1, i2, i3, i4, i5, i6, i7, s);
parameter width = 1;
output [width-1:0] y;
input  [width-1:0] i0;
input  [width-1:0] i1;
input  [width-1:0] i2;
input  [width-1:0] i3;
input  [width-1:0] i4;
input  [width-1:0] i5;
input  [width-1:0] i6;
input  [width-1:0] i7;
input        [2:0] s;
reg    [width-1:0] y;
always @(s or i0 or i1 or i2 or i3 or i4 or i5 or i6 or i7)
   case(s[2:0])
   0:       y = i0;
   1:       y = i1;
   2:       y = i2;
   3:       y = i3;
   4:       y = i4;
   5:       y = i5;
   6:       y = i6;
   7:       y = i7;
   default: y = {width{1'bx}};
   endcase
endmodule
module fp_muxd2 (y, i0, i1, s);
parameter width = 1;
output [width-1:0] y;
input  [width-1:0] i0;
input  [width-1:0] i1;
input        [1:0] s;
assign y = {width{s[0]}} & i0
         | {width{s[1]}} & i1;
endmodule
module fp_muxd3 (y, i0, i1, i2, s);
parameter width = 1;
output [width-1:0] y;
input  [width-1:0] i0;
input  [width-1:0] i1;
input  [width-1:0] i2;
input        [2:0] s;
assign y = {width{s[0]}} & i0
         | {width{s[1]}} & i1
         | {width{s[2]}} & i2;
endmodule
module fp_muxd4 (y, i0, i1, i2, i3, s);
parameter width = 1;
output [width-1:0] y;
input  [width-1:0] i0;
input  [width-1:0] i1;
input  [width-1:0] i2;
input  [width-1:0] i3;
input        [3:0] s;
assign y = {width{s[0]}} & i0
         | {width{s[1]}} & i1
         | {width{s[2]}} & i2
         | {width{s[3]}} & i3;
endmodule
module fp_muxd5 (y, i0, i1, i2, i3, i4, s);
parameter width = 1;
output [width-1:0] y;
input  [width-1:0] i0;
input  [width-1:0] i1;
input  [width-1:0] i2;
input  [width-1:0] i3;
input  [width-1:0] i4;
input        [4:0] s;
assign y = {width{s[0]}} & i0
         | {width{s[1]}} & i1
         | {width{s[2]}} & i2
         | {width{s[3]}} & i3
         | {width{s[4]}} & i4;
endmodule
module fp_muxd6 (y, i0, i1, i2, i3, i4, i5, s);
parameter width = 1;
output [width-1:0] y;
input  [width-1:0] i0;
input  [width-1:0] i1;
input  [width-1:0] i2;
input  [width-1:0] i3;
input  [width-1:0] i4;
input  [width-1:0] i5;
input        [5:0] s;
assign y = {width{s[0]}} & i0
         | {width{s[1]}} & i1
         | {width{s[2]}} & i2
         | {width{s[3]}} & i3
         | {width{s[4]}} & i4
         | {width{s[5]}} & i5;
endmodule
module fp_muxd7 (y, i0, i1, i2, i3, i4, i5, i6, s);
parameter width = 1;
output [width-1:0] y;
input  [width-1:0] i0;
input  [width-1:0] i1;
input  [width-1:0] i2;
input  [width-1:0] i3;
input  [width-1:0] i4;
input  [width-1:0] i5;
input  [width-1:0] i6;
input        [6:0] s;
assign y = {width{s[0]}} & i0
         | {width{s[1]}} & i1
         | {width{s[2]}} & i2
         | {width{s[3]}} & i3
         | {width{s[4]}} & i4
         | {width{s[5]}} & i5
         | {width{s[6]}} & i6;
endmodule
module fp_muxd8 (y, i0, i1, i2, i3, i4, i5, i6, i7, s);
parameter width = 1;
output [width-1:0] y;
input  [width-1:0] i0;
input  [width-1:0] i1;
input  [width-1:0] i2;
input  [width-1:0] i3;
input  [width-1:0] i4;
input  [width-1:0] i5;
input  [width-1:0] i6;
input  [width-1:0] i7;
input        [7:0] s;
assign y = {width{s[0]}} & i0
         | {width{s[1]}} & i1
         | {width{s[2]}} & i2
         | {width{s[3]}} & i3
         | {width{s[4]}} & i4
         | {width{s[5]}} & i5
         | {width{s[6]}} & i6
         | {width{s[7]}} & i7;
endmodule
module fp_mxi2 (yn, i0, i1, s);
parameter width = 1;
output [width-1:0] yn;
input  [width-1:0] i0;
input  [width-1:0] i1;
input              s;
assign yn = ~(s ? i1 : i0);
endmodule
module fp_benc (s, a, x2, m2, m1, m0);
output s, a, x2;
input m2, m1, m0;
wire m2n, m1n, m0n;
wire m1_or_m0, m1n_or_m0n, x2n;
  not  u0 (m1n, m1);
  not  u1 (m0n, m0);
  or   u3 (m1n_or_m0n, m1n, m0n);
  nand u4 (s, m2, m1n_or_m0n);
  or   u5 (m1_or_m0, m1, m0);
  nand u6 (a, m2n, m1_or_m0);
  xor  u7 (x2n, m1, m0);
  not  u8 (x2, x2n);
  not  u9 (m2n, m2);
endmodule
module fp_bmx (pp, x2, a, s, m1, m0);
output pp;
input x2, a, s, m1, m0;
wire p1, p2;
assign
  p1 = ~(m1 ?  a : s),
  p2 = ~(m0 ?  a : s),
  pp =  (x2 ? p2 : p1);
endmodule
module fp_add2 (s, co, a, b);
output s, co;
input  a, b;
  xor u0 (s, a, b);
  and u1 (co, a, b);
endmodule
module fp_add3 (s, co, a, b, c);
output s, co;
input a, b, c;
wire t2,t3,t4;
  xor u1 (s, a, b, c);
  and u2 (t2, a, b);
  and u3 (t3, a, c);
  and u4 (t4, b, c);
  or  u5 (co, t2, t3, t4);
endmodule
module fp_add4 (s, co, ko, a, b, c, d, ki);
output s, co, ko;
input a, b, c, d, ki;
wire  is,t2,t3,t4,t5,t6,t7;
  xor u0 (is, a, b, c);
  and u1 (t2, a, b);
  and u2 (t3, a, c);
  and u3 (t4, b, c);
  or  u4 (ko, t2, t3, t4);
  xor u5 (s, is, d, ki);
  and u6 (t5, is, d);
  and u7 (t6, is, ki);
  and u8 (t7, d, ki);
  or  u9 (co, t5, t6, t7);
endmodule
module fp_not(y,a);
input a;
output y;
not (y,a);
endmodule
module fp_buf(y,a);
input a;
output y;
buf (y,a);
endmodule
module fp_nand2(y,a,b);
input a,b;
output y;
nand (y,a,b);
endmodule
module fp_nand3(y,a,b,c);
input a,b,c;
output y;
nand (y,a,b,c);
endmodule
module fp_or2(y,a,b);
input a,b;
output y;
or (y,a,b);
endmodule
module fp_or3(y,a,b,c);
input a,b,c;
output y;
or (y,a,b,c);
endmodule
module fp_xnor2(y,a,b);
input a,b;
output y;
xnor (y,a,b);
endmodule
module fp_nor2(y,a,b);
input a,b;
output y;
nor (y,a,b);
endmodule
module fp_aoi21(y,a,b,c);
input a,b,c;
output y;
wire ab;
and (ab,a,b);
nor (y,ab,c);
endmodule
