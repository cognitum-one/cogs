// ******************************************************************
// fpcrtl: RTL files for fpc unit
//         stage, single-precision floating-point compare unit
// ******************************************************************
// $Id: fpcrtl.v 1.2 2021/10/10 22:26:10 gw Exp $
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

module A2F_c (  // renamed from fpcrtl.v //
   Fpc_Result_x1, Fpc_IF_x1, Fpc_DF_x1, Fpc_LT_x1, Fpc_GT_x1, Fpc_UO_x1,
   Fpc_Opcode_x1, Fpc_SrcA_x1, Fpc_SrcB_x1
);
output [31:0] Fpc_Result_x1;
output        Fpc_IF_x1;
output        Fpc_DF_x1;
output        Fpc_LT_x1;
output        Fpc_GT_x1;
output        Fpc_UO_x1;
input   [4:0] Fpc_Opcode_x1;
input  [31:0] Fpc_SrcA_x1;
input  [31:0] Fpc_SrcB_x1;
wire        eanz_x1;
wire        ebnz_x1;
wire        eano_x1;
wire        ebno_x1;
wire        manz_x1;
wire        mbnz_x1;
wire        fsel_x1;
wire        enab_x1;
wire        enba_x1;
wire  [1:0] cmsk_x1;
wire  [1:0] smsk_x1;
wire        fLT_x1;
wire        fGT_x1;
wire        cmpg_x1;
wire        cmpp_x1;
fpcot fpcot (
   .eanz (eanz_x1),
   .ebnz (ebnz_x1),
   .eano (eano_x1),
   .ebno (ebno_x1),
   .manz (manz_x1),
   .mbnz (mbnz_x1),
   .SrcA (Fpc_SrcA_x1[30:0]),
   .SrcB (Fpc_SrcB_x1[30:0])
);
fpcctl fpcctl (
   .fsel   (fsel_x1),
   .enab   (enab_x1),
   .enba   (enba_x1),
   .cmsk   (cmsk_x1[1:0]),
   .smsk   (smsk_x1[1:0]),
   .fLT    (fLT_x1),
   .fGT    (fGT_x1),
   .fUO    (Fpc_UO_x1),
   .IE     (Fpc_IF_x1),
   .DF     (Fpc_DF_x1),
   .opcode (Fpc_Opcode_x1[4:0]),
   .eanz   (eanz_x1),
   .eano   (eano_x1),
   .manz   (manz_x1),
   .ebnz   (ebnz_x1),
   .ebno   (ebno_x1),
   .mbnz   (mbnz_x1),
   .sgna   (Fpc_SrcA_x1[31]),
   .sgnb   (Fpc_SrcB_x1[31]),
   .mams   (Fpc_SrcA_x1[22]),
   .mbms   (Fpc_SrcB_x1[22])
);
fpccmp fpccmp (
   .cmpg (cmpg_x1),
   .cmpp (cmpp_x1),
   .SrcA (Fpc_SrcA_x1[30:0]),
   .SrcB (Fpc_SrcB_x1[30:0])
);
fpcrmx fpcrmx (
   .Fpc_Result (Fpc_Result_x1[31:0]),
   .Fpc_LT     (Fpc_LT_x1),
   .Fpc_GT     (Fpc_GT_x1),
   .SrcA       (Fpc_SrcA_x1[31:0]),
   .SrcB       (Fpc_SrcB_x1[31:0]),
   .tstop      (Fpc_Opcode_x1[2:0]),
   .fsel       (fsel_x1),
   .enab       (enab_x1),
   .enba       (enba_x1),
   .cmsk       (cmsk_x1[1:0]),
   .smsk       (smsk_x1[1:0]),
   .fLT        (fLT_x1),
   .fGT        (fGT_x1),
   .cmpg       (cmpg_x1),
   .cmpp       (cmpp_x1)
);
endmodule
module fpcctl
(  fsel, enab, enba, cmsk, smsk, fLT, fGT, fUO, IE, DF, opcode, eanz,
   eano, manz, ebnz, ebno, mbnz, sgna, sgnb, mams, mbms
);
output       fsel;
output       enab;
output       enba;
output [1:0] cmsk;
output [1:0] smsk;
output       fLT;
output       fGT;
output       fUO;
output       IE;
output       DF;
input  [4:0] opcode;
input        eanz;
input        eano;
input        manz;
input        ebnz;
input        ebno;
input        mbnz;
input        sgna;
input        sgnb;
input        mams;
input        mbms;
wire fsel;
wire enab;
wire enba;
wire DF;
wire IE;
wire fLT;
wire fGT;
wire fUO;
wire [1:0] cmsk;
wire [1:0] smsk;
   assign fsel = ~mbnz & ebnz & ~sgnb & ~eanz & opcode[0]
               | ebno & ebnz & ~sgnb & ~eanz & opcode[0]
               | ~ebnz & ~manz & sgna & opcode[0]
               | ~mbnz & ~sgnb & ~manz & sgna & opcode[0]
               | ebno & ~sgnb & ~manz & sgna & opcode[0]
               | ~ebnz & eano & sgna & opcode[0]
               | ~mbnz & ~sgnb & eano & sgna & opcode[0]
               | ebno & ~sgnb & eano & sgna & opcode[0]
               | mbms & manz & ~eano & ~opcode[3]
               | ~mbnz & manz & ~eano & ~opcode[3]
               | ebno & manz & ~eano & ~opcode[3]
               | ~mams & manz & ~eano & ~opcode[3]
               | mbms & mbnz & ~ebno & ~opcode[4]
               | ~mams & manz & ~eano & ~opcode[4]
               | ~mbnz & ebnz & sgnb & ~eanz & opcode[2] & ~opcode[4]
               | ebno & ebnz & sgnb & ~eanz & opcode[2] & ~opcode[4]
               | ~ebnz & ~manz & ~sgna & opcode[2] & ~opcode[4]
               | ~mbnz & sgnb & ~manz & ~sgna & opcode[2] & ~opcode[4]
               | ebno & sgnb & ~manz & ~sgna & opcode[2] & ~opcode[4]
               | ~ebnz & eano & ~sgna & opcode[2] & ~opcode[4]
               | ~mbnz & sgnb & eano & ~sgna & opcode[2] & ~opcode[4]
               | ebno & sgnb & eano & ~sgna & opcode[2] & ~opcode[4]
               | ~eanz & ~opcode[3] & ~opcode[4];
   assign enab = ~( mbnz & ~ebno
                  | manz & ~eano
                  | ~ebnz
                  | sgnb
                  | ~eanz
                  | sgna
                  | opcode[3] & opcode[4]
                  );
   assign enba = ~mbnz & ebnz & sgnb & ~manz & ~eano & sgna & ~opcode[4]
               | ebno & ebnz & sgnb & ~manz & ~eano & sgna & ~opcode[4]
               | ~mbnz & ebnz & sgnb & eano & eanz & sgna & ~opcode[4]
               | ebno & ebnz & sgnb & eano & eanz & sgna & ~opcode[4];
   assign cmsk[1] = ~mbnz & ~opcode[2] & opcode[3]
                  | ebno & ~opcode[2] & opcode[3]
                  | mbnz & ~ebno & ~opcode[3] & opcode[4]
                  | manz & ~eano & ~opcode[3] & opcode[4]
                  | ~eanz & ~opcode[3] & opcode[4]
                  | ~sgna & ~opcode[3] & opcode[4]
                  | opcode[3] & ~opcode[4];
   assign cmsk[0] = ~manz & eanz & sgna & opcode[0] & opcode[3]
                  | eano & eanz & sgna & opcode[0] & opcode[3]
                  | ~mbnz & ebnz & ~sgnb & ~opcode[0] & opcode[3]
                  | ebno & ebnz & ~sgnb & opcode[2] & opcode[3]
                  | ~manz & eanz & ~sgna & opcode[2] & opcode[3]
                  | eano & eanz & ~sgna & opcode[2] & opcode[3]
                  | ~mbnz & ebnz & sgnb & ~opcode[2] & opcode[3]
                  | ebno & ebnz & sgnb & ~opcode[2] & opcode[3]
                  | ebno & ~opcode[0] & opcode[4]
                  | opcode[2] & opcode[4]
                  | mbnz & ~ebno & ~opcode[3] & opcode[4]
                  | manz & ~eano & ~opcode[3] & opcode[4]
                  | ebnz & sgnb & ~eanz & ~opcode[3] & opcode[4]
                  | ebnz & eanz & ~sgna & ~opcode[3] & opcode[4]
                  | ~mbms & mbnz & ~ebno & opcode[3] & ~opcode[4]
                  | ~mams & manz & ~eano & opcode[3] & ~opcode[4]
                  | ebnz & eanz & opcode[3] & ~opcode[4];
   assign smsk[1] = mbnz & ~ebno & ~opcode[3] & opcode[4]
                  | manz & ~eano & ~opcode[3] & opcode[4]
                  | ~mbms & mbnz & ~ebno & opcode[3] & ~opcode[4]
                  | mbnz & ~ebno & manz & ~eano & opcode[3] & ~opcode[4]
                  | ~mams & manz & ~eano & opcode[3] & ~opcode[4];
   assign smsk[0] = ~( mbnz & ~ebno
                     | manz & ~eano
                     | ebnz & ~sgnb & ~eanz & ~opcode[0]
                     | ~ebnz & eanz & sgna & ~opcode[0]
                     | ~sgnb & eanz & sgna & ~opcode[0]
                     | ~ebnz & ~eanz & ~opcode[1]
                     | ebnz & sgnb & ~eanz & ~opcode[2]
                     | ~ebnz & eanz & ~sgna & ~opcode[2]
                     | sgnb & eanz & ~sgna & ~opcode[2]
                     | opcode[3]
                     | opcode[4]
                     );
   assign DF = mbnz & ~ebnz & ~opcode[3]
             | manz & ~eanz & ~opcode[3]
             | mbnz & ~ebnz & ~opcode[4]
             | manz & ~eanz & ~opcode[4];
   assign IE = ~mbms & mbnz & ~ebno & ~opcode[2]
             | ~mams & manz & ~eano & ~opcode[3]
             | ~mbms & mbnz & ~ebno & ~opcode[4]
             | ~mams & manz & ~eano & ~opcode[4];
   assign fLT = ~( mbnz & ~ebno
                 | manz & ~eano
                 | ebnz & sgnb
                 | ~ebnz & ~eanz
                 | eanz & ~sgna
                 );
   assign fGT = ~( mbnz & ~ebno
                 | manz & ~eano
                 | ebnz & ~sgnb
                 | ~ebnz & ~eanz
                 | eanz & sgna
                 );
   assign fUO = mbnz & ~ebno
              | manz & ~eano;
endmodule
module fpcot (
   eanz, ebnz, eano, ebno, manz, mbnz,
   SrcA, SrcB
);
output        eanz;
output        ebnz;
output        eano;
output        ebno;
output        manz;
output        mbnz;
input  [30:0] SrcA;
input  [30:0] SrcB;
assign
   eanz    =  (|SrcA[30:23]),
   ebnz    =  (|SrcB[30:23]),
   eano    = ~(&SrcA[30:23]),
   ebno    = ~(&SrcB[30:23]),
   manz    =  (|SrcA[22:0]),
   mbnz    =  (|SrcB[22:0]);
endmodule
module fpcrmx (
   Fpc_Result, Fpc_LT, Fpc_GT,
   SrcA, SrcB, tstop, fsel, enab, enba, cmsk, smsk, fLT, fGT, cmpg, cmpp
);
output [31:0] Fpc_Result;
output        Fpc_LT;
output        Fpc_GT;
input  [31:0] SrcA;
input  [31:0] SrcB;
input   [2:0] tstop;
input         fsel;
input         enab;
input         enba;
input   [1:0] cmsk;
input   [1:0] smsk;
input         fLT;
input         fGT;
input         cmpg;
input         cmpp;
wire  [2:0] tstopr;
wire  [2:0] top;
wire        tsel;
wire [31:0] res0;
assign
   tstopr[0]   = tstop[2],
   tstopr[1]   = tstop[1],
   tstopr[2]   = tstop[0],
   top[2:0] = {3{enab}} & tstop[2:0]
            | {3{enba}} & tstopr[2:0],
   tsel = top[0] & ~cmpg & ~cmpp
        | top[1] & ~cmpg &  cmpp
        | top[2] &  cmpg
        | fsel;
fp_mux2 #(32) mx_res0 (
   .y  (res0[31:0]),
   .i0 (SrcB[31:0]),
   .i1 (SrcA[31:0]),
   .s  (tsel)
);
assign Fpc_Result[31:0] = {cmsk[1],{31{cmsk[0]}}} & res0[31:0]
                        | {9'b0,smsk[1],21'b0, smsk[0] & tsel};
assign
   Fpc_LT = enab & ~cmpg & ~cmpp
          | enba &  cmpg
          | fLT,
   Fpc_GT = enab &  cmpg
          | enba & ~cmpg & ~cmpp
          | fGT;
endmodule
module fpccmp (cmpg, cmpp, SrcA, SrcB);
output        cmpg;
output        cmpp;
input  [30:0] SrcA;
input  [30:0] SrcB;
wire g00n  = ~(SrcA[00] & ~SrcB[00]);
wire g01n  = ~(SrcA[01] & ~SrcB[01]);
wire g02n  = ~(SrcA[02] & ~SrcB[02]);
wire g03n  = ~(SrcA[03] & ~SrcB[03]);
wire g04n  = ~(SrcA[04] & ~SrcB[04]);
wire g05n  = ~(SrcA[05] & ~SrcB[05]);
wire g06n  = ~(SrcA[06] & ~SrcB[06]);
wire g07n  = ~(SrcA[07] & ~SrcB[07]);
wire g08n  = ~(SrcA[08] & ~SrcB[08]);
wire g09n  = ~(SrcA[09] & ~SrcB[09]);
wire g10n  = ~(SrcA[10] & ~SrcB[10]);
wire g11n  = ~(SrcA[11] & ~SrcB[11]);
wire g12n  = ~(SrcA[12] & ~SrcB[12]);
wire g13n  = ~(SrcA[13] & ~SrcB[13]);
wire g14n  = ~(SrcA[14] & ~SrcB[14]);
wire g15n  = ~(SrcA[15] & ~SrcB[15]);
wire g16n  = ~(SrcA[16] & ~SrcB[16]);
wire g17n  = ~(SrcA[17] & ~SrcB[17]);
wire g18n  = ~(SrcA[18] & ~SrcB[18]);
wire g19n  = ~(SrcA[19] & ~SrcB[19]);
wire g20n  = ~(SrcA[20] & ~SrcB[20]);
wire g21n  = ~(SrcA[21] & ~SrcB[21]);
wire g22n  = ~(SrcA[22] & ~SrcB[22]);
wire g23n  = ~(SrcA[23] & ~SrcB[23]);
wire g24n  = ~(SrcA[24] & ~SrcB[24]);
wire g25n  = ~(SrcA[25] & ~SrcB[25]);
wire g26n  = ~(SrcA[26] & ~SrcB[26]);
wire g27n  = ~(SrcA[27] & ~SrcB[27]);
wire g28n  = ~(SrcA[28] & ~SrcB[28]);
wire g29n  = ~(SrcA[29] & ~SrcB[29]);
wire g30n  = ~(SrcA[30] & ~SrcB[30]);
wire p00n  = ~(SrcA[00] | ~SrcB[00]);
wire p01n  = ~(SrcA[01] | ~SrcB[01]);
wire p02n  = ~(SrcA[02] | ~SrcB[02]);
wire p03n  = ~(SrcA[03] | ~SrcB[03]);
wire p04n  = ~(SrcA[04] | ~SrcB[04]);
wire p05n  = ~(SrcA[05] | ~SrcB[05]);
wire p06n  = ~(SrcA[06] | ~SrcB[06]);
wire p07n  = ~(SrcA[07] | ~SrcB[07]);
wire p08n  = ~(SrcA[08] | ~SrcB[08]);
wire p09n  = ~(SrcA[09] | ~SrcB[09]);
wire p10n  = ~(SrcA[10] | ~SrcB[10]);
wire p11n  = ~(SrcA[11] | ~SrcB[11]);
wire p12n  = ~(SrcA[12] | ~SrcB[12]);
wire p13n  = ~(SrcA[13] | ~SrcB[13]);
wire p14n  = ~(SrcA[14] | ~SrcB[14]);
wire p15n  = ~(SrcA[15] | ~SrcB[15]);
wire p16n  = ~(SrcA[16] | ~SrcB[16]);
wire p17n  = ~(SrcA[17] | ~SrcB[17]);
wire p18n  = ~(SrcA[18] | ~SrcB[18]);
wire p19n  = ~(SrcA[19] | ~SrcB[19]);
wire p20n  = ~(SrcA[20] | ~SrcB[20]);
wire p21n  = ~(SrcA[21] | ~SrcB[21]);
wire p22n  = ~(SrcA[22] | ~SrcB[22]);
wire p23n  = ~(SrcA[23] | ~SrcB[23]);
wire p24n  = ~(SrcA[24] | ~SrcB[24]);
wire p25n  = ~(SrcA[25] | ~SrcB[25]);
wire p26n  = ~(SrcA[26] | ~SrcB[26]);
wire p27n  = ~(SrcA[27] | ~SrcB[27]);
wire p28n  = ~(SrcA[28] | ~SrcB[28]);
wire p29n  = ~(SrcA[29] | ~SrcB[29]);
wire p30n  = ~(SrcA[30] | ~SrcB[30]);
wire  g0100  = ~(g01n & (p01n | g00n));
wire  g0302  = ~(g03n & (p03n | g02n));
wire  g0504  = ~(g05n & (p05n | g04n));
wire  g0706  = ~(g07n & (p07n | g06n));
wire  g0908  = ~(g09n & (p09n | g08n));
wire  g1110  = ~(g11n & (p11n | g10n));
wire  g1312  = ~(g13n & (p13n | g12n));
wire  g1514  = ~(g15n & (p15n | g14n));
wire  g1716  = ~(g17n & (p17n | g16n));
wire  g1918  = ~(g19n & (p19n | g18n));
wire  g2120  = ~(g21n & (p21n | g20n));
wire  g2322  = ~(g23n & (p23n | g22n));
wire  g2524  = ~(g25n & (p25n | g24n));
wire  g2726  = ~(g27n & (p27n | g26n));
wire  g2928  = ~(g29n & (p29n | g28n));
wire  g30    = ~(g30n);
wire  p0100  = ~(p01n | p00n);
wire  p0302  = ~(p03n | p02n);
wire  p0504  = ~(p05n | p04n);
wire  p0706  = ~(p07n | p06n);
wire  p0908  = ~(p09n | p08n);
wire  p1110  = ~(p11n | p10n);
wire  p1312  = ~(p13n | p12n);
wire  p1514  = ~(p15n | p14n);
wire  p1716  = ~(p17n | p16n);
wire  p1918  = ~(p19n | p18n);
wire  p2120  = ~(p21n | p20n);
wire  p2322  = ~(p23n | p22n);
wire  p2524  = ~(p25n | p24n);
wire  p2726  = ~(p27n | p26n);
wire  p2928  = ~(p29n | p28n);
wire  p30    = ~(p30n);
wire g0300n  = ~(g0302 | (p0302 & g0100));
wire g0704n  = ~(g0706 | (p0706 & g0504));
wire g1108n  = ~(g1110 | (p1110 & g0908));
wire g1512n  = ~(g1514 | (p1514 & g1312));
wire g1916n  = ~(g1918 | (p1918 & g1716));
wire g2320n  = ~(g2322 | (p2322 & g2120));
wire g2724n  = ~(g2726 | (p2726 & g2524));
wire g3028n  = ~(g30   | (p30   & g2928));
wire p0300n  = ~(p0302 & p0100);
wire p0704n  = ~(p0706 & p0504);
wire p1108n  = ~(p1110 & p0908);
wire p1512n  = ~(p1514 & p1312);
wire p1916n  = ~(p1918 & p1716);
wire p2320n  = ~(p2322 & p2120);
wire p2724n  = ~(p2726 & p2524);
wire p3028n  = ~(p30   & p2928);
wire g0700  = ~(g0704n & (p0704n | g0300n));
wire g1508  = ~(g1512n & (p1512n | g1108n));
wire g2316  = ~(g2320n & (p2320n | g1916n));
wire g3024  = ~(g3028n & (p3028n | g2724n));
wire p0700  = ~(p0704n | p0300n);
wire p1508  = ~(p1512n | p1108n);
wire p2316  = ~(p2320n | p1916n);
wire p3024  = ~(p3028n | p2724n);
wire g1500n  = ~(g1508 | (p1508 & g0700));
wire g3016n  = ~(g3024 | (p3024 & g2316));
wire p1500n  = ~(p1508 & p0700);
wire p3016n  = ~(p3024 & p2316);
wire g3000  = ~(g3016n & (p3016n | g1500n));
wire p3000  = ~(p3016n | p1500n);
assign cmpg = g3000;
assign cmpp = p3000;
endmodule
