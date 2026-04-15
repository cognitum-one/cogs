// ******************************************************************
// fpm2rtl: RTL files for fpm unit - part 2
//         single-precision floating-point multiply/add unit
//         with 32x32 integer multiply operations
// ******************************************************************
// $Id: fpm2rtl.v 1.1 2021/10/10 20:45:31 gw Exp $
// ******************************************************************
// Copyright (c) GB3 Digital Systems, 2010-2021. All rights reserved.
// This source file is confidential and the proprietary
// information of GB3 Digital Systems and its receipt or
// possession does not convey any rights to reproduce or
// disclose its contents, or to manufacture, use or sell
// anything it may describe. Reproduction, disclosure or use
// without GB3 Digital Systems' specific written authorization
// is strictly forbidden.
// ******************************************************************

//`include "A2F_lib.v"
module A2F_m2 ( // renamed from fpm2rtl.v //
  result_x5, Fpm_OF_x5, Fpm_UF_x5, Fpm_XF_x5,
  nmr_x4, nr_x4, sadj_x4, e2m1_x4, e2p0_x4, e2p1_x4, ovfm1_x4, ovfp0_x4, ovfp1_x4, unfnm1_x4,
  unfnp0_x4, unfnp1_x4, ezm1_x4, ezp0_x4, mskm1_x4, mskp0_x4, mskp1_x4, rsgn_x4, mrz_x4,
  asel_x5, inc2_x5, ren_x5, fpop_x5
);
output [31:0]  result_x5;
output         Fpm_OF_x5;
output         Fpm_UF_x5;
output         Fpm_XF_x5;
input  [33:0]  nmr_x4;
input  [31:0]  nr_x4;
input          sadj_x4;
input   [7:0]  e2m1_x4;
input   [7:0]  e2p0_x4;
input   [7:0]  e2p1_x4;
input          ovfm1_x4;
input          ovfp0_x4;
input          ovfp1_x4;
input          unfnm1_x4;
input          unfnp0_x4;
input          unfnp1_x4;
input          ezm1_x4;
input          ezp0_x4;
input   [3:0]  mskm1_x4;
input   [3:0]  mskp0_x4;
input   [3:0]  mskp1_x4;
input          rsgn_x4;
input          mrz_x4;
input          asel_x5;
input          inc2_x5;
input    [2:0] ren_x5;
input          fpop_x5;
wire [33:0]  nmr_x5;
wire [31:0]  nr_x5;
wire         sadj_x5;
wire  [7:0]  e2m1_x5;
wire  [7:0]  e2p0_x5;
wire  [7:0]  e2p1_x5;
wire         ovfm1_x5;
wire         ovfp0_x5;
wire         ovfp1_x5;
wire         unfnm1_x5;
wire         unfnp0_x5;
wire         unfnp1_x5;
wire         ezm1_x5;
wire         ezp0_x5;
wire  [3:0]  mskm1_x5;
wire  [3:0]  mskp0_x5;
wire  [3:0]  mskp1_x5;
wire         rsgn_x5;
wire         mrz_x5;
wire [31:0] mr_x5;
wire        rovf_x5;
wire        dron_x5;
wire        xf_x5;
wire        ovf0_x5;
wire        ovf1_x5;
wire        unf0n_x5;
wire        unf1n_x5;
wire  [7:0] rexp0_x5;
wire  [7:0] rexp1_x5;
wire        cmsk0_x5;
wire        cmsk1_x5;
wire        smsk0_x5;
wire        smsk1_x5;
wire [31:0] result_x5;
wire        Fpm_OF_x5;
wire        Fpm_UF_x5;
wire        Fpm_XF_x5;
assign nr_x5[31:0]     = nr_x4[31:0];
assign nmr_x5[33:0]    = nmr_x4[33:0];
assign sadj_x5         = sadj_x4;
assign e2m1_x5[7:0]    = e2m1_x4[7:0];
assign e2p0_x5[7:0]    = e2p0_x4[7:0];
assign e2p1_x5[7:0]    = e2p1_x4[7:0];
assign ovfm1_x5        = ovfm1_x4;
assign ovfp0_x5        = ovfp0_x4;
assign ovfp1_x5        = ovfp1_x4;
assign unfnm1_x5       = unfnm1_x4;
assign unfnp0_x5       = unfnp0_x4;
assign unfnp1_x5       = unfnp1_x4;
assign ezm1_x5         = ezm1_x4;
assign ezp0_x5         = ezp0_x4;
assign mskm1_x5[3:0]   = mskm1_x4[3:0];
assign mskp0_x5[3:0]   = mskp0_x4[3:0];
assign mskp1_x5[3:0]   = mskp1_x4[3:0];
assign rsgn_x5         = rsgn_x4;
assign mrz_x5          = mrz_x4;
fpmrnd fpmrnd (
   .mr   (mr_x5[31:0]),
   .rovf (rovf_x5),
   .dron (dron_x5),
   .xf   (xf_x5),
   .inc2 (inc2_x5),
   .ren  (ren_x5[2:0]),
   .nmr  (nmr_x5[33:0])
);
fpme5 fpme5 (
   .ovf0   (ovf0_x5),
   .ovf1   (ovf1_x5),
   .unf0n  (unf0n_x5),
   .unf1n  (unf1n_x5),
   .rexp0  (rexp0_x5[7:0]),
   .rexp1  (rexp1_x5[7:0]),
   .cmsk0  (cmsk0_x5),
   .cmsk1  (cmsk1_x5),
   .smsk0  (smsk0_x5),
   .smsk1  (smsk1_x5),
   .e2m1   (e2m1_x5[7:0]),
   .e2p0   (e2p0_x5[7:0]),
   .e2p1   (e2p1_x5[7:0]),
   .ovfm1  (ovfm1_x5),
   .ovfp0  (ovfp0_x5),
   .ovfp1  (ovfp1_x5),
   .unfnm1 (unfnm1_x5),
   .unfnp0 (unfnp0_x5),
   .unfnp1 (unfnp1_x5),
   .ezm1   (ezm1_x5),
   .ezp0   (ezp0_x5),
   .mskm1  (mskm1_x5[3:0]),
   .mskp0  (mskp0_x5[3:0]),
   .mskp1  (mskp1_x5[3:0]),
   .sadj   (sadj_x5),
   .dron   (dron_x5),
   .asel   (asel_x5),
   .fpop   (fpop_x5),
   .nr     (nr_x5[30:23])
);
fpmrmx fpmrmx (
   .result (result_x5[31:0]),
   .Fpm_OF (Fpm_OF_x5),
   .Fpm_UF (Fpm_UF_x5),
   .Fpm_XF (Fpm_XF_x5),
   .mr     (mr_x5[31:0]),
   .rsgn   (rsgn_x5),
   .nrsgn  (nr_x5[31]),
   .nr     (nr_x5[22:0]),
   .ovf0   (ovf0_x5),
   .ovf1   (ovf1_x5),
   .unf0n  (unf0n_x5),
   .unf1n  (unf1n_x5),
   .rexp0  (rexp0_x5[7:0]),
   .rexp1  (rexp1_x5[7:0]),
   .cmsk0  (cmsk0_x5),
   .cmsk1  (cmsk1_x5),
   .smsk0  (smsk0_x5),
   .smsk1  (smsk1_x5),
   .xf     (xf_x5),
   .mrz    (mrz_x5),
   .fpop   (fpop_x5),
   .asel   (asel_x5),
   .rovf   (rovf_x5)
);
endmodule
module fpmrnd (
   mr, rovf, dron, xf,
   inc2, ren, nmr
);
output [31:0] mr;
output        rovf;
output        dron;
output        xf;
input         inc2;
input   [2:0] ren;
input  [33:0] nmr;
wire          rnd;
assign rnd = ren[0] & nmr[1] & (nmr[2] | nmr[0])
           | ren[1] & (nmr[1] | nmr[0])
           | ren[2];
wire  drnd = ren[0] & nmr[2]
           | ren[1] & (nmr[2] | nmr[1] | nmr[0]);
assign xf  = nmr[1] | nmr[0];
wire c01n_1 = ~(nmr[01] | inc2);
wire c02n_1 = ~(nmr[02] & (nmr[01] | inc2));
wire p0302n = ~(nmr[03] & nmr[02]);
wire p0403n = ~(nmr[04] & nmr[03]);
wire p0504n = ~(nmr[05] & nmr[04]);
wire p0605n = ~(nmr[06] & nmr[05]);
wire p0706n = ~(nmr[07] & nmr[06]);
wire p0807n = ~(nmr[08] & nmr[07]);
wire p0908n = ~(nmr[09] & nmr[08]);
wire p1009n = ~(nmr[10] & nmr[09]);
wire p1110n = ~(nmr[11] & nmr[10]);
wire p1211n = ~(nmr[12] & nmr[11]);
wire p1312n = ~(nmr[13] & nmr[12]);
wire p1413n = ~(nmr[14] & nmr[13]);
wire p1514n = ~(nmr[15] & nmr[14]);
wire p1615n = ~(nmr[16] & nmr[15]);
wire p1716n = ~(nmr[17] & nmr[16]);
wire p1817n = ~(nmr[18] & nmr[17]);
wire p1918n = ~(nmr[19] & nmr[18]);
wire p2019n = ~(nmr[20] & nmr[19]);
wire p2120n = ~(nmr[21] & nmr[20]);
wire p2221n = ~(nmr[22] & nmr[21]);
wire p2322n = ~(nmr[23] & nmr[22]);
wire p2423n = ~(nmr[24] & nmr[23]);
wire p2524n = ~(nmr[25] & nmr[24]);
wire p2625n = ~(nmr[26] & nmr[25]);
wire p2726n = ~(nmr[27] & nmr[26]);
wire p2827n = ~(nmr[28] & nmr[27]);
wire p2928n = ~(nmr[29] & nmr[28]);
wire p3029n = ~(nmr[30] & nmr[29]);
wire p3130n = ~(nmr[31] & nmr[30]);
wire p3231n = ~(nmr[32] & nmr[31]);
wire c01_2 = ~(c01n_1);
wire c02_2 = ~(c02n_1);
wire c03_2 = ~(p0302n | c01n_1);
wire c04_2 = ~(p0403n | c02n_1);
wire p0502 = ~(p0504n | p0302n);
wire p0603 = ~(p0605n | p0403n);
wire p0704 = ~(p0706n | p0504n);
wire p0805 = ~(p0807n | p0605n);
wire p0906 = ~(p0908n | p0706n);
wire p1007 = ~(p1009n | p0807n);
wire p1108 = ~(p1110n | p0908n);
wire p1209 = ~(p1211n | p1009n);
wire p1310 = ~(p1312n | p1110n);
wire p1411 = ~(p1413n | p1211n);
wire p1512 = ~(p1514n | p1312n);
wire p1613 = ~(p1615n | p1413n);
wire p1714 = ~(p1716n | p1514n);
wire p1815 = ~(p1817n | p1615n);
wire p1916 = ~(p1918n | p1716n);
wire p2017 = ~(p2019n | p1817n);
wire p2118 = ~(p2120n | p1918n);
wire p2219 = ~(p2221n | p2019n);
wire p2320 = ~(p2322n | p2120n);
wire p2421 = ~(p2423n | p2221n);
wire p2522 = ~(p2524n | p2322n);
wire p2623 = ~(p2625n | p2423n);
wire p2724 = ~(p2726n | p2524n);
wire p2825 = ~(p2827n | p2625n);
wire p2926 = ~(p2928n | p2726n);
wire p3027 = ~(p3029n | p2827n);
wire p3128 = ~(p3130n | p2928n);
wire p3229 = ~(p3231n | p3029n);
wire c01n_3 = ~(c01_2);
wire c02n_3 = ~(c02_2);
wire c03n_3 = ~(c03_2);
wire c04n_3 = ~(c04_2);
wire c05n_3 = ~(p0502 & c01_2);
wire c06n_3 = ~(p0603 & c02_2);
wire c07n_3 = ~(p0704 & c03_2);
wire c08n_3 = ~(p0805 & c04_2);
wire p0902n = ~(p0906 & p0502);
wire p1003n = ~(p1007 & p0603);
wire p1104n = ~(p1108 & p0704);
wire p1205n = ~(p1209 & p0805);
wire p1306n = ~(p1310 & p0906);
wire p1407n = ~(p1411 & p1007);
wire p1508n = ~(p1512 & p1108);
wire p1609n = ~(p1613 & p1209);
wire p1710n = ~(p1714 & p1310);
wire p1811n = ~(p1815 & p1411);
wire p1912n = ~(p1916 & p1512);
wire p2013n = ~(p2017 & p1613);
wire p2114n = ~(p2118 & p1714);
wire p2215n = ~(p2219 & p1815);
wire p2316n = ~(p2320 & p1916);
wire p2417n = ~(p2421 & p2017);
wire p2518n = ~(p2522 & p2118);
wire p2619n = ~(p2623 & p2219);
wire p2720n = ~(p2724 & p2320);
wire p2821n = ~(p2825 & p2421);
wire p2922n = ~(p2926 & p2522);
wire p3023n = ~(p3027 & p2623);
wire p3124n = ~(p3128 & p2724);
wire p3225n = ~(p3229 & p2825);
wire c01_4 = ~(c01n_3);
wire c02_4 = ~(c02n_3);
wire c03_4 = ~(c03n_3);
wire c04_4 = ~(c04n_3);
wire c05_4 = ~(c05n_3);
wire c06_4 = ~(c06n_3);
wire c07_4 = ~(c07n_3);
wire c08_4 = ~(c08n_3);
wire c09_4 = ~(p0902n | c01n_3);
wire c10_4 = ~(p1003n | c02n_3);
wire c11_4 = ~(p1104n | c03n_3);
wire c12_4 = ~(p1205n | c04n_3);
wire c13_4 = ~(p1306n | c05n_3);
wire c14_4 = ~(p1407n | c06n_3);
wire c15_4 = ~(p1508n | c07n_3);
wire c16_4 = ~(p1609n | c08n_3);
wire p1702 = ~(p1710n | p0902n);
wire p1803 = ~(p1811n | p1003n);
wire p1904 = ~(p1912n | p1104n);
wire p2005 = ~(p2013n | p1205n);
wire p2106 = ~(p2114n | p1306n);
wire p2207 = ~(p2215n | p1407n);
wire p2308 = ~(p2316n | p1508n);
wire p2409 = ~(p2417n | p1609n);
wire p2510 = ~(p2518n | p1710n);
wire p2611 = ~(p2619n | p1811n);
wire p2712 = ~(p2720n | p1912n);
wire p2813 = ~(p2821n | p2013n);
wire p2914 = ~(p2922n | p2114n);
wire p3015 = ~(p3023n | p2215n);
wire p3116 = ~(p3124n | p2316n);
wire p3217 = ~(p3225n | p2417n);
wire p2518 = ~(p2518n);
wire c01n_5 = ~(c01_4 & rnd);
wire c02n_5 = ~(c02_4 & rnd);
wire c03n_5 = ~(c03_4 & rnd);
wire c04n_5 = ~(c04_4 & rnd);
wire c05n_5 = ~(c05_4 & rnd);
wire c06n_5 = ~(c06_4 & rnd);
wire c07n_5 = ~(c07_4 & rnd);
wire c08n_5 = ~(c08_4 & rnd);
wire c19n_5 = ~(c09_4 & rnd);
wire c10n_5 = ~(c10_4 & rnd);
wire c11n_5 = ~(c11_4 & rnd);
wire c12n_5 = ~(c12_4 & rnd);
wire c13n_5 = ~(c13_4 & rnd);
wire c14n_5 = ~(c14_4 & rnd);
wire c15n_5 = ~(c15_4 & rnd);
wire c16n_5 = ~(c16_4 & rnd);
wire c17n_5 = ~(p1702 & c01_4 & rnd);
wire c18n_5 = ~(p1803 & c02_4 & rnd);
wire c29n_5 = ~(p1904 & c03_4 & rnd);
wire c20n_5 = ~(p2005 & c04_4 & rnd);
wire c21n_5 = ~(p2106 & c05_4 & rnd);
wire c22n_5 = ~(p2207 & c06_4 & rnd);
wire c23n_5 = ~(p2308 & c07_4 & rnd);
wire c24n_5 = ~(p2409 & c08_4 & rnd);
wire c25n_5 = ~(p2510 & c09_4 & rnd);
wire c26n_5 = ~(p2611 & c10_4 & rnd);
wire c27n_5 = ~(p2712 & c11_4 & rnd);
wire c28n_5 = ~(p2813 & c12_4 & rnd);
wire c39n_5 = ~(p2914 & c13_4 & rnd);
wire c30n_5 = ~(p3015 & c14_4 & rnd);
wire c31n_5 = ~(p3116 & c15_4 & rnd);
wire c32n_5 = ~(p3217 & c16_4 & rnd);
wire dron   = ~(p2518 & p1803 & drnd);
assign mr[00] = ~(c01n_5 ^ nmr[02]);
assign mr[01] = ~(c02n_5 ^ nmr[03]);
assign mr[02] = ~(c03n_5 ^ nmr[04]);
assign mr[03] = ~(c04n_5 ^ nmr[05]);
assign mr[04] = ~(c05n_5 ^ nmr[06]);
assign mr[05] = ~(c06n_5 ^ nmr[07]);
assign mr[06] = ~(c07n_5 ^ nmr[08]);
assign mr[07] = ~(c08n_5 ^ nmr[09]);
assign mr[08] = ~(c19n_5 ^ nmr[10]);
assign mr[09] = ~(c10n_5 ^ nmr[11]);
assign mr[10] = ~(c11n_5 ^ nmr[12]);
assign mr[11] = ~(c12n_5 ^ nmr[13]);
assign mr[12] = ~(c13n_5 ^ nmr[14]);
assign mr[13] = ~(c14n_5 ^ nmr[15]);
assign mr[14] = ~(c15n_5 ^ nmr[16]);
assign mr[15] = ~(c16n_5 ^ nmr[17]);
assign mr[16] = ~(c17n_5 ^ nmr[18]);
assign mr[17] = ~(c18n_5 ^ nmr[19]);
assign mr[18] = ~(c29n_5 ^ nmr[20]);
assign mr[19] = ~(c20n_5 ^ nmr[21]);
assign mr[20] = ~(c21n_5 ^ nmr[22]);
assign mr[21] = ~(c22n_5 ^ nmr[23]);
assign mr[22] = ~(c23n_5 ^ nmr[24]);
assign mr[23] = ~(c24n_5 ^ nmr[25]);
assign mr[24] = ~(c25n_5 ^ nmr[26]);
assign mr[25] = ~(c26n_5 ^ nmr[27]);
assign mr[26] = ~(c27n_5 ^ nmr[28]);
assign mr[27] = ~(c28n_5 ^ nmr[29]);
assign mr[28] = ~(c39n_5 ^ nmr[30]);
assign mr[29] = ~(c30n_5 ^ nmr[31]);
assign mr[30] = ~(c31n_5 ^ nmr[32]);
assign mr[31] = ~(c32n_5 ^ nmr[33]);
assign rovf  = ~c25n_5;
endmodule
module fpme5 (
   ovf0, ovf1, unf0n, unf1n, rexp0, rexp1, cmsk0, cmsk1, smsk0, smsk1,
   e2m1, e2p0, e2p1, ovfm1, ovfp0, ovfp1, unfnm1, unfnp0, unfnp1,
   ezm1, ezp0, mskm1, mskp0, mskp1, sadj, dron, asel, fpop, nr
);
output        ovf0;
output        ovf1;
output        unf0n;
output        unf1n;
output  [7:0] rexp0;
output  [7:0] rexp1;
output        cmsk0;
output        cmsk1;
output        smsk0;
output        smsk1;
input   [7:0] e2m1;
input   [7:0] e2p0;
input   [7:0] e2p1;
input         ovfm1;
input         ovfp0;
input         ovfp1;
input         unfnm1;
input         unfnp0;
input         unfnp1;
input         ezm1;
input         ezp0;
input   [3:0] mskm1;
input   [3:0] mskp0;
input   [3:0] mskp1;
input         sadj;
input         dron;
input         asel;
input         fpop;
input [30:23] nr;
wire  [3:0] msk0;
wire  [3:0] msk1;
wire  [7:0] exp0;
wire  [7:0] exp1;
wire        dunf;
assign
   ovf0      =    asel   & (sadj ? ovfm1      : ovfp0),
   ovf1      =    asel   & (sadj ? ovfp0      : ovfp1),
   unf0n     =   ~asel   | (sadj ? unfnm1     : unfnp0),
   unf1n     =   ~asel   | (sadj ? unfnp0     : unfnp1),
   msk0[3:0] = {4{asel}} & (sadj ? mskm1[3:0] : mskp0[3:0]),
   msk1[3:0] = {4{asel}} & (sadj ? mskp0[3:0] : mskp1[3:0]),
   exp0[7:0] =             (sadj ? e2m1[7:0]  : e2p0[7:0]),
   exp1[7:0] =             (sadj ? e2p0[7:0]  : e2p1[7:0]),
   dunf      = asel & ~dron & (sadj ? ezm1 : ezp0);
assign
   rexp0[7:0] = ( exp0[7:0] & {8{msk0[3]}}
                | {{7{msk0[2]}},msk0[1] | dunf}
                | nr[30:23]
                ),
   rexp1[7:0] = ( exp1[7:0] & {8{msk1[3]}}
                | {{7{msk1[2]}},msk1[1]}
                | nr[30:23]
                );
assign
   cmsk0      = asel & msk0[3] | ~fpop,
   cmsk1      = asel & msk1[3] | ~fpop,
   smsk0      = asel & msk0[0],
   smsk1      = asel & msk1[0];
endmodule
module fpmrmx (
   result, Fpm_OF, Fpm_UF, Fpm_XF,
   mr, rsgn, nrsgn, nr, ovf0, ovf1, unf0n, unf1n, rexp0, rexp1,
   cmsk0, cmsk1, smsk0, smsk1, xf, mrz, fpop, asel, rovf
);
output [31:0] result;
output        Fpm_OF;
output        Fpm_UF;
output        Fpm_XF;
input  [31:0] mr;
input         rsgn;
input         nrsgn;
input  [22:0] nr;
input         ovf0;
input         ovf1;
input         unf0n;
input         unf1n;
input   [7:0] rexp0;
input   [7:0] rexp1;
input         cmsk0;
input         cmsk1;
input         smsk0;
input         smsk1;
input         xf;
input         mrz;
input         fpop;
input         asel;
input         rovf;
wire        sgn;
wire  [7:0] rexp;
wire [31:0] rman;
wire        cmsk;
wire        smsk;
wire [22:0] mmsk;
wire        mxf;
wire        ovf;
wire        unf;
assign
   rexp[7:0] = rovf ? rexp1[7:0] : rexp0[7:0],
   sgn       = asel ? rsgn : nrsgn;
assign
   cmsk       = rovf ? cmsk1 : cmsk0,
   smsk       = rovf ? smsk1 : smsk0,
   mmsk[22:00] = {23{smsk}} | nr[22:00],
   rman[22:00] = {23{cmsk}} & mr[22:00] | mmsk[22:00],
   rman[31:23] = {9{cmsk}}  & mr[31:23];
assign
    result[31:23] = fpop  ? {sgn, rexp[7:0]} : rman[31:23],
    result[22:0]  = rman[22:0];
assign
   ovf  =  (rovf ? ovf1  : ovf0),
   unf  = ~(rovf ? unf1n : unf0n),
   mxf  = xf & asel,
   Fpm_OF =   ovf,
   Fpm_UF =   unf & (~mrz | mxf),
   Fpm_XF = ((unf & ~mrz) | mxf | ovf);
endmodule
