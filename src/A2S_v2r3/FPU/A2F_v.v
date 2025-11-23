// ******************************************************************
// fpvrtl: RTL files for fpv unit
//         single-precision floating-point conversion unit
// ******************************************************************
// $Id: fpvrtl.v 2.0 2021/03/15 03:24:58 gw Exp $
// ******************************************************************
// Copyright (c) GB3 Digital Systems, 2011-2021. All rights reserved.
// This source file is confidential and the proprietary
// information of GB3 Digital Systems and its receipt or
// possession does not convey any rights to reproduce or
// disclose its contents, or to manufacture, use or sell
// anything it may describe. Reproduction, disclosure or use
// without GB3 Digital Systems' specific written authorization
// is strictly forbidden.
// ******************************************************************
//`include "A2F_lib.v"

module A2F_v (  // renamed from fpvrtl.v //
   Fpv_Result_x2, Fpv_IF_x2, Fpv_XF_x2, Fpv_DF_x2,
   Fpv_Opcode_x1, Fpv_Rm_x1, Fpv_Src_x1
);
output [31:0] Fpv_Result_x2;  // data result
output        Fpv_IF_x2;      // invalid operation flag
output        Fpv_XF_x2;      // inexact flag
output        Fpv_DF_x2;      // denormal flag
input   [1:0] Fpv_Opcode_x1;  // operation code (0=f2s, 1=s2f, 2=f2u, 3=u2f)
input   [1:0] Fpv_Rm_x1;      // rounding mode
input  [31:0] Fpv_Src_x1;     // Source operand
wire [31:0] ir_x1;       // integer result before rounding
wire        inc_x1;      // increment result
wire        rndXF_x1;    // round inexact
wire        iresDF_x1;   // denormal flag
wire        toobig_x1;   // operand to big for conversion
wire        isgn_x1;     // sign of integer result
wire        fsgn_x1;     // sign of float result
wire [29:0] nmr2_x1;     // partially normalized manitssa
wire        grd8_x1;     // guard bit if nsc=8
wire  [4:0] nsc_x1;      // normalize shift count
wire        srczn_x1;    // integer source is zero
wire  [1:0] ren_x1;      // round enables
wire  [1:0] opcode_x2 ;  // operation
wire [31:0] ir_x2;       // integer result before rounding
wire        inc_x2;      // increment result
wire        rndXF_x2;    // round inexact
wire        iresDF_x2;   // denormal flag
wire        toobig_x2;   // operand to big for conversion
wire        isgn_x2;     // sign of integer result
wire        fsgn_x2;     // sign of float result
wire [29:0] nmr2_x2;     // partially normalized manitssa
wire        grd8_x2;     // guard bit if nsc=8
wire  [4:0] nsc_x2;      // normalize shift count
wire        srczn_x2;    // integer source is zero
wire  [1:0] ren_x2;      // round enables
wire [31:0] ires_x2;     // integer result
wire        iresIF_x2;   // integer result: invalid operation flag
wire        iresXF_x2;   // integer result: inexact flag
wire [31:0] fres_x2;     // floating-point result
wire        fresXF_x2;   // floating-point result: inexact flag
fpvint1 fpvint1 (
   .ir     (ir_x1[31:0]),
   .inc    (inc_x1),
   .rndXF  (rndXF_x1),
   .resDF  (iresDF_x1),
   .toobig (toobig_x1),
   .sgn    (isgn_x1),
   .Src    (Fpv_Src_x1[31:0]),
   .rm     (Fpv_Rm_x1[1:0]),
   .usgn   (Fpv_Opcode_x1[1])
);
fpvflt1 fpvflt1 (
   .sgn   (fsgn_x1),
   .nmr2  (nmr2_x1[29:0]),
   .grd8  (grd8_x1),
   .nsc   (nsc_x1[4:0]),
   .srczn (srczn_x1),
   .ren   (ren_x1[1:0]),
   .src   (Fpv_Src_x1[31:0]),
   .rm    (Fpv_Rm_x1[1:0]),
   .usgn   (Fpv_Opcode_x1[1])
);
assign opcode_x2[1:0] = Fpv_Opcode_x1[1:0];
assign ir_x2[31:0]    = ir_x1[31:0];
assign inc_x2         = inc_x1;
assign rndXF_x2       = rndXF_x1;
assign iresDF_x2      = iresDF_x1;
assign toobig_x2      = toobig_x1;
assign isgn_x2        = isgn_x1;
assign nmr2_x2[29:0]  = nmr2_x1[29:0];
assign grd8_x2        = grd8_x1;
assign nsc_x2[4:0]    = nsc_x1[4:0];
assign fsgn_x2        = fsgn_x1;
assign srczn_x2       = srczn_x1;
assign ren_x2[1:0]    = ren_x1[1:0];
fpvint2 fpvint2 (
   .res    (ires_x2[31:0]),
   .resIF  (iresIF_x2),
   .resXF  (iresXF_x2),
   .ir     (ir_x2[31:0]),
   .inc    (inc_x2),
   .rndXF  (rndXF_x2),
   .toobig (toobig_x2),
   .sgn    (isgn_x2),
   .usgn   (opcode_x2[1])
);
fpvflt2 fpvflt2 (
   .res     (fres_x2[31:0]),
   .resXF   (fresXF_x2),
   .sgn     (fsgn_x2),
   .nmr2    (nmr2_x2[29:0]),
   .grd8    (grd8_x2),
   .nsc     (nsc_x2[4:0]),
   .srczn   (srczn_x2),
   .ren     (ren_x2[1:0])
);
assign
   Fpv_Result_x2[31:0] =  opcode_x2[0] ? fres_x2[31:0] : ires_x2[31:0],
   Fpv_XF_x2           =  opcode_x2[0] ? fresXF_x2     : iresXF_x2,
   Fpv_IF_x2           = ~opcode_x2[0] & iresIF_x2,
   Fpv_DF_x2           = ~opcode_x2[0] & iresDF_x2;
endmodule // fpv
module fpvint1 (
   ir, inc, rndXF, resDF, toobig, sgn,
   Src, rm, usgn
);
output [31:0] ir;     // integer result before rounding
output        inc;    // increment result
output        rndXF;  // round inexact
output        resDF;  // denormal operand
output        toobig; // operand to big for conversion
output        sgn;    // sign of result
input  [31:0] Src;    // source operand
input   [1:0] rm;     // round mode
input         usgn;   // unsigned operation
wire        expzn;    // exponent != 0
wire        manzn;    // mantissa != 0
wire        toosmall; // |Src| < 2^-1
wire        toobig;   // |Src| > 2^31
reg  [33:0] dm;       // denormalized mantissa
wire  [1:0] ren;      // round enables
wire        rnd;      // round decision
wire        inc;      // increment result
wire        srnd;     // small operand rounds to integer 1
wire [31:0] ir0;      // integer result (after too small mux)
wire [31:0] ir;       // integer result (after invert, before round inc)
assign
   expzn    =  (|Src[30:23]),
   manzn    =  (|Src[22:0]),
   sgn      =  expzn & Src[31],
   resDF    = ~expzn & manzn,
   toosmall = (Src[30:23] < 'h7e),
   toobig   = (Src[30:23] > 'h9e)
            | usgn & sgn;
always @(Src)                                             // --- exponent---
   casez(Src[28:23])                                      // unbiased biased
   6'b011110: dm[33:0] = { 1'b1, Src[22:00],      10'b0}; //       31   'h9e
   6'b011101: dm[33:0] = { 2'b1, Src[22:00],       9'b0}; //       30   'h9d
   6'b011100: dm[33:0] = { 3'b1, Src[22:00],       8'b0}; //       29   'h9c
   6'b011011: dm[33:0] = { 4'b1, Src[22:00],       7'b0}; //       28   'h9b
   6'b011010: dm[33:0] = { 5'b1, Src[22:00],       6'b0}; //       27   'h9a
   6'b011001: dm[33:0] = { 6'b1, Src[22:00],       5'b0}; //       26   'h99
   6'b011000: dm[33:0] = { 7'b1, Src[22:00],       4'b0}; //       25   'h98
   6'b010111: dm[33:0] = { 8'b1, Src[22:00],       3'b0}; //       24   'h97
   6'b010110: dm[33:0] = { 9'b1, Src[22:00],       2'b0}; //       23   'h96
   6'b010101: dm[33:0] = {10'b1, Src[22:00],       1'b0}; //       22   'h95
   6'b010100: dm[33:0] = {11'b1, Src[22:01],  Src[00]  }; //       21   'h94
   6'b010011: dm[33:0] = {12'b1, Src[22:02], |Src[01:0]}; //       20   'h93
   6'b010010: dm[33:0] = {13'b1, Src[22:03], |Src[02:0]}; //       19   'h92
   6'b010001: dm[33:0] = {14'b1, Src[22:04], |Src[03:0]}; //       18   'h91
   6'b010000: dm[33:0] = {15'b1, Src[22:05], |Src[04:0]}; //       17   'h90
   6'b001111: dm[33:0] = {16'b1, Src[22:06], |Src[05:0]}; //       16   'h8f
   6'b001110: dm[33:0] = {17'b1, Src[22:07], |Src[06:0]}; //       15   'h8e
   6'b001101: dm[33:0] = {18'b1, Src[22:08], |Src[07:0]}; //       14   'h8d
   6'b001100: dm[33:0] = {19'b1, Src[22:09], |Src[08:0]}; //       13   'h8c
   6'b001011: dm[33:0] = {20'b1, Src[22:10], |Src[09:0]}; //       12   'h8b
   6'b001010: dm[33:0] = {21'b1, Src[22:11], |Src[10:0]}; //       11   'h8a
   6'b001001: dm[33:0] = {22'b1, Src[22:12], |Src[11:0]}; //       10   'h89
   6'b001000: dm[33:0] = {23'b1, Src[22:13], |Src[12:0]}; //       09   'h88
   6'b000111: dm[33:0] = {24'b1, Src[22:14], |Src[13:0]}; //       08   'h87
   6'b000110: dm[33:0] = {25'b1, Src[22:15], |Src[14:0]}; //       07   'h86
   6'b000101: dm[33:0] = {26'b1, Src[22:16], |Src[15:0]}; //       06   'h85
   6'b000100: dm[33:0] = {27'b1, Src[22:17], |Src[16:0]}; //       05   'h84
   6'b000011: dm[33:0] = {28'b1, Src[22:18], |Src[17:0]}; //       04   'h83
   6'b000010: dm[33:0] = {29'b1, Src[22:19], |Src[18:0]}; //       03   'h82
   6'b000001: dm[33:0] = {30'b1, Src[22:20], |Src[19:0]}; //       02   'h81
   6'b000000: dm[33:0] = {31'b1, Src[22:21], |Src[20:0]}; //       01   'h80
   6'b111111: dm[33:0] = {32'b1, Src[22]   , |Src[21:0]}; //       00   'h7f
   6'b111110: dm[33:0] = {33'b1            , |Src[22:0]}; //       -1   'h7e
   default:   dm[33:0] =  34'bx;
   endcase
assign
   ren[0] = (rm[1:0] ==2'b00),
   ren[1] = (rm[1:0] ==2'b01) &  Src[31]
          | (rm[1:0] ==2'b10) & ~Src[31],
   rnd    = ren[0] & dm[1] & (dm[2] | dm[0])
          | ren[1] & (dm[1] | dm[0]),
   inc    = sgn ^ (rnd & ~toosmall),
   srnd   = ren[1] & expzn,                     // small number rounds to 1
   rndXF  = (dm[1] | dm[0] | toosmall) & expzn; // inexact (inhibit if operand = 0)
assign
   ir0[31:0] = toosmall ? {31'b0,srnd} : dm[33:2], // unsigned integer result
   ir[31:0]  = {32{sgn}} ^ ir0[31:0];
endmodule // fpvint1
module fpvint2 (
   res, resIF, resXF,
   ir, inc, rndXF, toobig, sgn, usgn
);
output [31:0] res;    // integer result
output        resIF;  // result flags: invalid operation
output        resXF;  // result flags: inexact
input  [31:0] ir;     // integer result before rounding
input         inc;    // increment result
input         rndXF;  // round inexact
input         toobig; // operand to big for conversion
input         sgn;    // result sign
input         usgn;   // unsigned operation
wire [31:0] rres;  // rounded integer result
wire        ovf;   // magnitude of integer result is too big
wire        co;    // increment carry out (result==0)
assign
   {co,rres[31:0]} = ir[31:0]+{31'b0, inc},
   ovf             = ~sgn &  rres[31] & ~usgn // pos result too big
                   |  sgn & ~rres[31] & ~co;  // neg result too big (inhibit if zero)
assign
   resIF = ( toobig   // operand > 2^32 or infinity or NaN
           | ovf      // +result >= +2^31 or -result < -2^31
           ),
   resXF = rndXF & ~resIF;
assign
   res[31:0] = resIF ? 32'h80000000 // invalid operation result
                     : rres[31:0];  // arithmetic result
endmodule // fpvint2
module fpvflt1 (sgn, nmr2, grd8, nsc, srczn, ren, src, rm, usgn);
output        sgn;   // source sign
output [29:0] nmr2;  // partially normalized manitssa ([29:3] inverted)
output        grd8;  // guard bit if nsc=8
output  [4:0] nsc;   // normalize shift count
output        srczn; // source is zero
output  [1:0] ren;   // round enables
input  [31:0] src;   // source operand
input   [1:0] rm;    // round mode
input         usgn;  // unsigned operation
wire [31:0] dmr;   // integer absolute value (denormalized manitssa)
assign
   sgn   = src[31] & ~usgn,
   srczn = |src[31:0];
assign
   ren[0] = (rm[1:0] ==2'b00),
   ren[1] = (rm[1:0] ==2'b01) &  sgn
          | (rm[1:0] ==2'b10) & ~sgn;
assign
   dmr[31:0] = sgn ? -src[31:0] : src[31:0];
fpvlbc fpvlbc (
   .nsc (nsc[4:0]),
   .src (src[31:1]),
   .sgn (sgn)
);
fpvnsh1 fpvnsh1 (
   .nmr2 (nmr2[29:0]),
   .grd8 (grd8),
   .nsc  (nsc[4:0]),
   .dmr  (dmr[31:0])
);
endmodule // fpvflt1
module fpvflt2 (
   res, resXF,
   sgn, nmr2, nsc, grd8, srczn, ren
);
output [31:0] res;   // result
output        resXF; // result inexact flag
input         sgn;   // source sign
input  [29:0] nmr2;  // partially normalized manitssa
input         grd8;  // guard bit if nsc=8
input   [4:0] nsc;   // normalize shift count
input         srczn; // source is zero
input   [1:0] ren;   // round enables
wire [26:0] nmr;    // normalized manitssa
wire        sticky; // sticky bit
wire        guard;  // guard bit (1/2 bit)
wire        lsb;    // LSB of unrounded mantissa
wire        resXF;  // inexact flag
wire        rnd;    // round decision
wire [25:3] nmi;    // incremented nmr
wire        co;     // increment carry out
reg  [22:0] man;    // manitssa result
reg         adj;    // exponent adjust
wire  [7:0] exp0;   // exponent if no round adjustment
wire  [7:0] exp1;   // exponent if round adjustment
wire  [7:0] gexp0;  // gated exp0 (0 if zero)
wire  [7:0] gexp1;  // gated exp1 (0 if zero)
wire  [7:0] exp;    // exponent result
fpvnsh2 fpvnsh2 (
   .nmr   (nmr[26:0]),
   .nscls (nsc[1:0]),
   .nscms (nsc[4:3]),
   .nmr2  (nmr2[29:0]),
   .grd8  (grd8)
);
assign {co, nmi[25:3]} = nmr[25:3] + 24'b1;
assign
   sticky  = nmr[26] ? |nmr[1:0] : nmr[0],
   guard   = nmr[26] ?  nmr[2]   : nmr[1],
   lsb     = nmr[26] ?  nmr[3]   : nmr[2],
   resXF   = guard | sticky,
   rnd     = ren[0] & guard & (lsb   | sticky)
           | ren[1] & (guard | sticky);
assign
  exp0[7:0] = 8'h9f + {3'b111,~nsc[4:0]},
  exp1[7:0] = 8'h9f + {3'b111,~nsc[4:0]} + 8'b1,
  gexp0[7:0] = {8{srczn}} & exp0[7:0],
  gexp1[7:0] = {8{srczn}} & exp1[7:0];
always @(nmr or nmi or rnd or co)
   casez ({nmr[26], nmr[2], rnd})
   3'b0?0: {adj, man[22:0]} = {1'b0, nmr[24:2]};       // <  1.0, no round
   3'b001: {adj, man[22:0]} = {1'b0, nmr[24:3], 1'b1}; // <  1.0, round, lsb=0
   3'b011: {adj, man[22:0]} = {  co, nmi[24:3], 1'b0}; // <  1.0, round, lsb=1
   3'b1?0: {adj, man[22:0]} = {1'b1, nmr[25:3]      }; // >= 1.0, no round
   3'b1?1: {adj, man[22:0]} = {1'b1, nmi[25:3]      }; // >= 1.0, round
   default:{adj, man[22:0]} = 24'bx;
   endcase
assign exp[7:0]  = adj ? gexp1[7:0] : gexp0[7:0];
assign res[31:0] = {sgn, exp[7:0], man[22:0]};
endmodule //fpvflt2
module fpvnsh1 (nmr2, grd8, nsc, dmr);
output [29:0] nmr2;  // partly normalized result ([29:3] inverted)
output        grd8;  // guard bit if nsc=8
input   [4:0] nsc;   // normalize shift count
input  [31:0] dmr;   // denormalized input
wire [32:0] s4; // shift by 16 result (inverted)
wire [32:2] s3; // shift by 8 result
wire [32:6] s2; // shift by 4 result (inverted)
reg   [2:0] ls; // ls shift result
assign
   s4[32:0] = ~(nsc[4] ? {dmr[16:0],16'b0} : {1'b0,dmr[31:0]}),
   s3[32:2] = ~(nsc[3] ? { s4[24:0],6'h3f} :  s4[32:2]),
   s2[32:6] = ~(nsc[2] ?   s3[28:2]        :  s3[32:6]),
   nmr2[29:3] = s2[32:6];
always @(dmr or nsc)
   case(nsc[2:0])   // guard   round,   sticky
   3'd 0:   ls[2:0] = {dmr[8], dmr[7], |dmr[6:0]};
   3'd 1:   ls[2:0] = {dmr[7], dmr[6], |dmr[5:0]};
   3'd 2:   ls[2:0] = {dmr[6], dmr[5], |dmr[4:0]};
   3'd 3:   ls[2:0] = {dmr[5], dmr[4], |dmr[3:0]};
   3'd 4:   ls[2:0] = {dmr[4], dmr[3], |dmr[2:0]};
   3'd 5:   ls[2:0] = {dmr[3], dmr[2], |dmr[1:0]};
   3'd 6:   ls[2:0] = {dmr[2], dmr[1], |dmr[0]  };
   3'd 7:   ls[2:0] = {dmr[1], dmr[0],    1'b0  };
   default: ls[2:0] = 3'bx;
   endcase
assign nmr2[2:0] = ls[2:0];
assign grd8 = (nsc[2:0]==3'b0) & dmr[0];
endmodule // fpvnsh1
module fpvnsh2 (nmr, nscls, nscms, nmr2, grd8);
output [26:0] nmr;   // normalized result
input   [1:0] nscls; // normalize shift count, [1:0]
input   [4:3] nscms; // normalize shift count, [4:3]
input  [29:0] nmr2;  // partly normalized result (inverted)
input         grd8;  // guard bit if nsc=8
wire [32:6] s2 = nmr2[29:3];
wire [32:8] s1;
wire [32:9] s0;
assign
   s1[32:8] = ~(nscls[1] ? s2[30:6] : s2[32:8]),
   s0[32:9] = ~(nscls[0] ? s1[31:8] : s1[32:9]),
   nmr[26:3] = ~s0[32:9];
assign
   nmr[2]    = ~nscms[4] & ~nscms[3] & nmr2[2]
             | ~nscms[4] &  nscms[3] & grd8,
   nmr[1:0]  = {2{~nscms[4] & ~nscms[3]}} & nmr2[1:0];
endmodule // fpvnsh2
module fpvlbc (nsc, src, sgn);
output  [4:0] nsc;   // normalize shift count
input  [31:1] src;   // input data
input         sgn;   // sign
wire z0_0300 = ~((~src[02] & src[01]) | src[03]);
wire z0_0704 = ~((~src[06] & src[05]) | src[07]);
wire z0_1108 = ~((~src[10] & src[09]) | src[11]);
wire z0_1512 = ~((~src[14] & src[13]) | src[15]);
wire z0_1916 = ~((~src[18] & src[17]) | src[19]);
wire z0_2320 = ~((~src[22] & src[21]) | src[23]);
wire z0_2724 = ~((~src[26] & src[25]) | src[27]);
wire z0_3128 = ~((~src[30] & src[29]) | src[31]);
wire z0604n =  (src[06] | src[04]);
wire z1008n =  (src[10] | src[08]);
wire z1412n =  (src[14] | src[12]);
wire z1816n =  (src[18] | src[16]);
wire z2220n =  (src[22] | src[20]);
wire z2624n =  (src[26] | src[24]);
wire z3028n =  (src[30] | src[28]);
wire z0302  = ~(src[03] | src[02]);
wire z0504  = ~(src[05] | src[04]);
wire z0706  = ~(src[07] | src[06]);
wire z0908  = ~(src[09] | src[08]);
wire z1110  = ~(src[11] | src[10]);
wire z1312  = ~(src[13] | src[12]);
wire z1514  = ~(src[15] | src[14]);
wire z1716  = ~(src[17] | src[16]);
wire z1918  = ~(src[19] | src[18]);
wire z2120  = ~(src[21] | src[20]);
wire z2322  = ~(src[23] | src[22]);
wire z2524  = ~(src[25] | src[24]);
wire z2726  = ~(src[27] | src[26]);
wire z2928  = ~(src[29] | src[28]);
wire z3130  = ~(src[31] | src[30]);
wire z0504n = ~z0504;
wire z0908n = ~z0908;
wire z1312n = ~z1312;
wire z1716n = ~z1716;
wire z2120n = ~z2120;
wire z2524n = ~z2524;
wire z2928n = ~z2928;
wire o0_0300 = ~((src[02] & ~src[01]) | ~src[03]);
wire o0_0704 = ~((src[06] & ~src[05]) | ~src[07]);
wire o0_1108 = ~((src[10] & ~src[09]) | ~src[11]);
wire o0_1512 = ~((src[14] & ~src[13]) | ~src[15]);
wire o0_1916 = ~((src[18] & ~src[17]) | ~src[19]);
wire o0_2320 = ~((src[22] & ~src[21]) | ~src[23]);
wire o0_2724 = ~((src[26] & ~src[25]) | ~src[27]);
wire o0_3128 = ~((src[30] & ~src[29]) | ~src[31]);
wire o0604n =  ~(src[06] & src[04]);
wire o1008n =  ~(src[10] & src[08]);
wire o1412n =  ~(src[14] & src[12]);
wire o1816n =  ~(src[18] & src[16]);
wire o2220n =  ~(src[22] & src[20]);
wire o2624n =  ~(src[26] & src[24]);
wire o3028n =  ~(src[30] & src[28]);
wire o0302  =  (src[03] & src[02]);
wire o0504n = ~(src[05] & src[04]);
wire o0706  =  (src[07] & src[06]);
wire o0908n = ~(src[09] & src[08]);
wire o1110  =  (src[11] & src[10]);
wire o1312n = ~(src[13] & src[12]);
wire o1514  =  (src[15] & src[14]);
wire o1716n = ~(src[17] & src[16]);
wire o1918  =  (src[19] & src[18]);
wire o2120n = ~(src[21] & src[20]);
wire o2322  =  (src[23] & src[22]);
wire o2524n = ~(src[25] & src[24]);
wire o2726  =  (src[27] & src[26]);
wire o2928n = ~(src[29] & src[28]);
wire o3130  =  (src[31] & src[30]);
wire o0504  = ~o0504n;
wire o0908  = ~o0908n;
wire o1312  = ~o1312n;
wire o1716  = ~o1716n;
wire o2120  = ~o2120n;
wire o2524  = ~o2524n;
wire o2928  = ~o2928n;
wire z0_0700n = ~((z0604n | z0_0300) & z0_0704);
wire z0_1508n = ~((z1412n | z0_1108) & z0_1512);
wire z0_2316n = ~((z2220n | z0_1916) & z0_2320);
wire z0_3124n = ~((z3028n | z0_2724) & z0_3128);
wire z1_0700n = ~((z0504n | z0302) & z0706);
wire z1_1508n = ~((z1312n | z1110) & z1514);
wire z1_2316n = ~((z2120n | z1918) & z2322);
wire z1_3124n = ~((z2928n | z2726) & z3130);
wire z1408  = ~(z1412n | z1008n);
wire z2216  = ~(z2220n | z1816n);
wire z3024  = ~(z3028n | z2624n);
wire z1308  = ~(z1312n | z0908n);
wire z2116  = ~(z2120n | z1716n);
wire z2924  = ~(z2928n | z2524n);
wire z0704n = ~(z0706 & z0504);
wire z1108n = ~(z1110 & z0908);
wire z1512n = ~(z1514 & z1312);
wire z1916n = ~(z1918 & z1716);
wire z2320n = ~(z2322 & z2120);
wire z2724n = ~(z2726 & z2524);
wire z3128n = ~(z3130 & z2928);
wire z1108  = ~z1108n;
wire z1916  = ~z1916n;
wire z2724  = ~z2724n;
wire o0_0700n = ~((o0604n | o0_0300) & o0_0704);
wire o0_1508n = ~((o1412n | o0_1108) & o0_1512);
wire o0_2316n = ~((o2220n | o0_1916) & o0_2320);
wire o0_3124n = ~((o3028n | o0_2724) & o0_3128);
wire o1_0700n = ~((o0504n | o0302) & o0706);
wire o1_1508n = ~((o1312n | o1110) & o1514);
wire o1_2316n = ~((o2120n | o1918) & o2322);
wire o1_3124n = ~((o2928n | o2726) & o3130);
wire o1408  = ~(o1412n | o1008n);
wire o2216  = ~(o2220n | o1816n);
wire o3024  = ~(o3028n | o2624n);
wire o1308  = ~(o1312n | o0908n);
wire o2116  = ~(o2120n | o1716n);
wire o2924  = ~(o2928n | o2524n);
wire o0704n = ~(o0706 & o0504);
wire o1108n = ~(o1110 & o0908);
wire o1512n = ~(o1514 & o1312);
wire o1916n = ~(o1918 & o1716);
wire o2320n = ~(o2322 & o2120);
wire o2724n = ~(o2726 & o2524);
wire o3128n = ~(o3130 & o2928);
wire o1108  = ~o1108n;
wire o1916  = ~o1916n;
wire o2724  = ~o2724n;
wire z0_1500 = ~((z1408 & z0_0700n) | z0_1508n);
wire z0_3116 = ~((z3024 & z0_2316n) | z0_3124n);
wire z1_1500 = ~((z1308 & z1_0700n) | z1_1508n);
wire z1_3116 = ~((z2924 & z1_2316n) | z1_3124n);
wire z2_1500 = ~((z1108 & z0704n) | z1512n);
wire z2_3116 = ~((z2724 & z2320n) | z3128n);
wire z3016n = ~(z3024 & z2216);
wire z2916n = ~(z2924 & z2116);
wire z2716n = ~(z2724 & z1916);
wire z1508  = ~(z1512n | z1108n);
wire z2316  = ~(z2320n | z1916n);
wire z3124  = ~(z3128n | z2724n);
wire z2316n = ~z2316;
wire o0_1500 = ~((o1408 & o0_0700n) | o0_1508n);
wire o0_3116 = ~((o3024 & o0_2316n) | o0_3124n);
wire o1_1500 = ~((o1308 & o1_0700n) | o1_1508n);
wire o1_3116 = ~((o2924 & o1_2316n) | o1_3124n);
wire o2_1500 = ~((o1108 & o0704n) | o1512n);
wire o2_3116 = ~((o2724 & o2320n) | o3128n);
wire o3016n = ~(o3024 & o2216);
wire o2916n = ~(o2924 & o2116);
wire o2716n = ~(o2724 & o1916);
wire o1508  = ~(o1512n | o1108n);
wire o2316  = ~(o2320n | o1916n);
wire o3124  = ~(o3128n | o2724n);
wire o2316n = ~o2316;
wire  [4:0] zcntn; // leading zeros count (inverted)
wire  [4:0] ocntn; // leading ones count (inverted)
assign
   zcntn[0]  = ~((z3016n | z0_1500) & z0_3116),
   zcntn[1]  = ~((z2916n | z1_1500) & z1_3116),
   zcntn[2]  = ~((z2716n | z2_1500) & z2_3116),
   zcntn[3]  = ~((z2316n |   z1508) &   z3124),
   zcntn[4]  = ~(z3124 & z2316);
assign
   ocntn[0]  = ~((o3016n | o0_1500) & o0_3116),
   ocntn[1]  = ~((o2916n | o1_1500) & o1_3116),
   ocntn[2]  = ~((o2716n | o2_1500) & o2_3116),
   ocntn[3]  = ~((o2316n |   o1508) &   o3124),
   ocntn[4]  = ~(o3124 & o2316);
assign nsc[4:0] = ~(sgn ? ocntn[4:0] : zcntn[4:0]);
endmodule // fpvlbc
