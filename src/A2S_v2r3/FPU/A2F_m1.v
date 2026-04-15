// ******************************************************************
// fpm1rtl: RTL files for fpm unit - part 1
//         single-precision floating-point multiply/add unit
//         with 32x32 integer multiply operations
// ******************************************************************
// $Id: fpm1rtl.v 1.1 2021/10/10 20:45:12 gw Exp $
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
module A2F_m1 (  // renamed from fpm1rtl.v //
  rsgn_x4, imul_x4, nmr_x4, nr_x4, sadj_x4, e2m1_x4, e2p0_x4, e2p1_x4, ovfm1_x4, ovfp0_x4, ovfp1_x4,
  unfnm1_x4, unfnp0_x4, unfnp1_x4, ezm1_x4, ezp0_x4, mskm1_x4, mskp0_x4, mskp1_x4, mrz_x4, asel_x4,
  inc2_x4, ren_x4, fpop_x4, IF_x4, DF_x4,
  Fpm_Opcode, Fpm_Rm, Fpm_SrcA, Fpm_SrcB, Fpm_SrcC
);
output         rsgn_x4;
output [63:0]  imul_x4;
output [33:0]  nmr_x4;
output [31:0]  nr_x4;
output         sadj_x4;
output  [7:0]  e2m1_x4;
output  [7:0]  e2p0_x4;
output  [7:0]  e2p1_x4;
output         ovfm1_x4;
output         ovfp0_x4;
output         ovfp1_x4;
output         unfnm1_x4;
output         unfnp0_x4;
output         unfnp1_x4;
output         ezm1_x4;
output         ezp0_x4;
output  [3:0]  mskm1_x4;
output  [3:0]  mskp0_x4;
output  [3:0]  mskp1_x4;
output         mrz_x4;
output         asel_x4;
output         inc2_x4;
output   [2:0] ren_x4;
output         fpop_x4;
output         IF_x4;
output         DF_x4;
input    [2:0] Fpm_Opcode;
input    [1:0] Fpm_Rm;
input   [31:0] Fpm_SrcA;
input   [31:0] Fpm_SrcB;
input   [31:0] Fpm_SrcC;
wire        mad_x1;
wire  [2:0] ppop_x1;
wire  [2:0] tf_x2;
wire        sgnf_x2;
wire        passQNAN_x2;
wire        rm1_x3;
wire        fpop_x3;
wire        fpop_x4;
wire        infsel_x4;
wire        asel_x4;
wire        inc2_x4;
wire  [2:0] ren_x4;
wire        IF_x4;
wire        DF_x4;
wire         sgna_x1;
wire         sgnb_x1;
wire         sgnc_x1;
wire         eanz_x1;
wire         ebnz_x1;
wire         ecnz_x1;
wire         eano_x1;
wire         ebno_x1;
wire         ecno_x1;
wire         manz_x1;
wire         mbnz_x1;
wire         mcnz_x1;
wire         mams_x1;
wire         mbms_x1;
wire         mcms_x1;
wire         rsgn_x4;
wire [63:0]  imul_x4;
wire [33:0]  nmr_x4;
wire [31:0]  nr_x4;
wire         sadj_x4;
wire  [7:0]  e2m1_x4;
wire  [7:0]  e2p0_x4;
wire  [7:0]  e2p1_x4;
wire         ovfm1_x4;
wire         ovfp0_x4;
wire         ovfp1_x4;
wire         unfnm1_x4;
wire         unfnp0_x4;
wire         unfnp1_x4;
wire         ezm1_x4;
wire         ezp0_x4;
wire  [3:0]  mskm1_x4;
wire  [3:0]  mskp0_x4;
wire  [3:0]  mskp1_x4;
wire         mrz_x4;
fpmctl fpmctl (
   .mad_x1         (mad_x1),
   .ppop_x1        (ppop_x1[2:0]),
   .tf_x2          (tf_x2[2:0]),
   .sgnf_x2        (sgnf_x2),
   .passQNAN_x2    (passQNAN_x2),
   .rm1_x3         (rm1_x3),
   .fpop_x3        (fpop_x3),
   .fpop_x4        (fpop_x4),
   .infsel_x4      (infsel_x4),
   .asel_x4        (asel_x4),
   .inc2_x4        (inc2_x4),
   .ren_x4         (ren_x4[2:0]),
   .IF_x4          (IF_x4),
   .DF_x4          (DF_x4),
   .Fpm_Opcode_x1  (Fpm_Opcode[2:0]),
   .Fpm_Rm_x1      (Fpm_Rm[1:0]),
   .sgna_x1        (sgna_x1),
   .sgnb_x1        (sgnb_x1),
   .sgnc_x1        (sgnc_x1),
   .eanz_x1        (eanz_x1),
   .ebnz_x1        (ebnz_x1),
   .ecnz_x1        (ecnz_x1),
   .eano_x1        (eano_x1),
   .ebno_x1        (ebno_x1),
   .ecno_x1        (ecno_x1),
   .manz_x1        (manz_x1),
   .mbnz_x1        (mbnz_x1),
   .mcnz_x1        (mcnz_x1),
   .mams_x1        (mams_x1),
   .mbms_x1        (mbms_x1),
   .mcms_x1        (mcms_x1),
   .rsgn_x4        (rsgn_x4)
);
fpmdp1 fpmdp1 (
   .sgna_x1     (sgna_x1),
   .sgnb_x1     (sgnb_x1),
   .sgnc_x1     (sgnc_x1),
   .eanz_x1     (eanz_x1),
   .ebnz_x1     (ebnz_x1),
   .ecnz_x1     (ecnz_x1),
   .eano_x1     (eano_x1),
   .ebno_x1     (ebno_x1),
   .ecno_x1     (ecno_x1),
   .manz_x1     (manz_x1),
   .mbnz_x1     (mbnz_x1),
   .mcnz_x1     (mcnz_x1),
   .mams_x1     (mams_x1),
   .mbms_x1     (mbms_x1),
   .mcms_x1     (mcms_x1),
   .rsgn_x4     (rsgn_x4),
   .imul_x4     (imul_x4[63:0]),
   .nmr_x4      (nmr_x4[33:0]),
   .nr_x4       (nr_x4[31:0]),
   .sadj_x4     (sadj_x4),
   .e2m1_x4     (e2m1_x4[7:0]),
   .e2p0_x4     (e2p0_x4[7:0]),
   .e2p1_x4     (e2p1_x4[7:0]),
   .ovfm1_x4    (ovfm1_x4),
   .ovfp0_x4    (ovfp0_x4),
   .ovfp1_x4    (ovfp1_x4),
   .unfnm1_x4   (unfnm1_x4),
   .unfnp0_x4   (unfnp0_x4),
   .unfnp1_x4   (unfnp1_x4),
   .ezm1_x4     (ezm1_x4),
   .ezp0_x4     (ezp0_x4),
   .mskm1_x4    (mskm1_x4[3:0]),
   .mskp0_x4    (mskp0_x4[3:0]),
   .mskp1_x4    (mskp1_x4[3:0]),
   .mrz_x4      (mrz_x4),
   .SrcA_x1     (Fpm_SrcA[31:0]),
   .SrcB_x1     (Fpm_SrcB[31:0]),
   .SrcC_x1     (Fpm_SrcC[31:0]),
   .mad_x1      (mad_x1),
   .ppop_x1     (ppop_x1[2:0]),
   .tf_x2       (tf_x2[2:0]),
   .sgnf_x2     (sgnf_x2),
   .passQNAN_x2 (passQNAN_x2),
   .rm1_x3      (rm1_x3),
   .fpop_x3     (fpop_x3),
   .fpop_x4     (fpop_x4),
   .infsel_x4   (infsel_x4)
);
endmodule
module fpmctl (
   mad_x1, ppop_x1, tf_x2, sgnf_x2, passQNAN_x2,
   rm1_x3, fpop_x3, fpop_x4, infsel_x4, asel_x4, inc2_x4,
   ren_x4, IF_x4, DF_x4,
   Fpm_Opcode_x1, Fpm_Rm_x1, sgna_x1, sgnb_x1,
   sgnc_x1, eanz_x1, ebnz_x1, ecnz_x1, eano_x1, ebno_x1, ecno_x1, manz_x1,
   mbnz_x1, mcnz_x1, mams_x1, mbms_x1, mcms_x1, rsgn_x4
);
output        mad_x1;
output  [2:0] ppop_x1;
output  [2:0] tf_x2;
output        sgnf_x2;
output        passQNAN_x2;
output        rm1_x3;
output        fpop_x3;
output        fpop_x4;
output        infsel_x4;
output        asel_x4;
output        inc2_x4;
output  [2:0] ren_x4;
output        IF_x4;
output        DF_x4;
input   [2:0] Fpm_Opcode_x1;
input   [1:0] Fpm_Rm_x1;
input         sgna_x1;
input         sgnb_x1;
input         sgnc_x1;
input         eanz_x1;
input         ebnz_x1;
input         ecnz_x1;
input         eano_x1;
input         ebno_x1;
input         ecno_x1;
input         manz_x1;
input         mbnz_x1;
input         mcnz_x1;
input         mams_x1;
input         mbms_x1;
input         mcms_x1;
input         rsgn_x4;
`define FPM_FMUL   3'b000
`define FPM_FMAD   3'b001
`define FPM_IMUL   3'b1??
`define FPM_IMULUU 3'b100
`define FPM_IMULUS 3'b101
`define FPM_IMULSU 3'b110
`define FPM_IMULSS 3'b111
wire  [2:0] ppop_x1;
wire        mad_x1;
wire  [2:0] opcode_x2;
wire  [1:0] rm_x2;
wire        sgna_x2;
wire        sgnb_x2;
wire        sgnc_x2;
wire        eanz_x2;
wire        ebnz_x2;
wire        ecnz_x2;
wire        eano_x2;
wire        ebno_x2;
wire        ecno_x2;
wire        manz_x2;
wire        mbnz_x2;
wire        mcnz_x2;
wire        mams_x2;
wire        mbms_x2;
wire        mcms_x2;
wire        rm1_x2;
wire        sgnf_x2;
wire        asel_x2;
wire  [2:0] tf_x2;
wire        passQNAN_x2;
wire        IF_x2;
wire        DF_x2;
wire        fpop_x2;
wire        fmad_x2;
wire  [2:0] opcode_x3;
wire  [1:0] rm_x3;
wire        asel_x3;
wire        IF_x3;
wire        DF_x3;
wire        fpop_x3;
wire        rm1_x3;
wire  [2:0] opcode_x4;
wire  [1:0] rm_x4;
wire        fpop_x4;
wire        asel_x4;
wire        IF_x4;
wire        DF_x4;
reg         inc2_x4;
reg   [2:0] ren_x4;
wire        infsel_x4;
assign ppop_x1[2:0] = Fpm_Opcode_x1[2] ? Fpm_Opcode_x1[2:0] : 3'b0;
assign mad_x1 = ~Fpm_Opcode_x1[2] & Fpm_Opcode_x1[0];
assign opcode_x2[2:0] = Fpm_Opcode_x1[2:0];
assign rm_x2[1:0]     = Fpm_Rm_x1[1:0];
assign sgna_x2        = sgna_x1;
assign sgnb_x2        = sgnb_x1;
assign sgnc_x2        = sgnc_x1;
assign eanz_x2        = eanz_x1;
assign eano_x2        = eano_x1;
assign manz_x2        = manz_x1;
assign mams_x2        = mams_x1;
assign ebnz_x2        = ebnz_x1;
assign ebno_x2        = ebno_x1;
assign mbnz_x2        = mbnz_x1;
assign mbms_x2        = mbms_x1;
assign ecnz_x2        = ecnz_x1;
assign ecno_x2        = ecno_x1;
assign mcnz_x2        = mcnz_x1;
assign mcms_x2        = mcms_x1;
assign rm1_x2 = (rm_x2[1:0]==2'b01);
fpmoa fpmoa
(  .fpop     (fpop_x2),
   .fmad     (fmad_x2),
   .eanz     (eanz_x2),
   .eano     (eano_x2),
   .manz     (manz_x2),
   .ebnz     (ebnz_x2),
   .ebno     (ebno_x2),
   .mbnz     (mbnz_x2),
   .ecnz     (ecnz_x2),
   .ecno     (ecno_x2),
   .mcnz     (mcnz_x2),
   .sgna     (sgna_x2),
   .sgnb     (sgnb_x2),
   .sgnc     (sgnc_x2),
   .mams     (mams_x2),
   .mbms     (mbms_x2),
   .mcms     (mcms_x2),
   .RM1      (rm1_x2),
   .sgnf     (sgnf_x2),
   .asel     (asel_x2),
   .tf       (tf_x2[2:0]),
   .passQNAN (passQNAN_x2),
   .IE       (IF_x2),
   .DF       (DF_x2)
);
assign fpop_x2 = ~opcode_x2[2];
assign fmad_x2 = ~opcode_x2[2] & opcode_x2[0];
assign opcode_x3[2:0] = opcode_x2[2:0];
assign rm_x3[1:0]     = rm_x2[1:0];
assign asel_x3        = asel_x2;
assign IF_x3          = IF_x2;
assign DF_x3          = DF_x2;
assign fpop_x3        = fpop_x2;
assign rm1_x3 = (rm_x3[1:0] == 2'b01);
assign opcode_x4[2:0] = opcode_x3[2:0];
assign rm_x4[1:0]     = rm_x3[1:0];
assign asel_x4        = asel_x3;
assign fpop_x4        = fpop_x3;
assign IF_x4          = IF_x3;
assign DF_x4          = DF_x3;
always @(opcode_x4 or rm_x4 or rsgn_x4)
  casez({opcode_x4,rm_x4,rsgn_x4})
  { `FPM_FMUL,3'b00_?}:{inc2_x4,ren_x4[2:0]} = 4'b1_001;
  { `FPM_FMUL,3'b01_0}:{inc2_x4,ren_x4[2:0]} = 4'b1_000;
  { `FPM_FMUL,3'b01_1}:{inc2_x4,ren_x4[2:0]} = 4'b1_010;
  { `FPM_FMUL,3'b10_0}:{inc2_x4,ren_x4[2:0]} = 4'b1_010;
  { `FPM_FMUL,3'b10_1}:{inc2_x4,ren_x4[2:0]} = 4'b1_000;
  { `FPM_FMUL,3'b11_?}:{inc2_x4,ren_x4[2:0]} = 4'b1_000;
  { `FPM_FMAD,3'b00_?}:{inc2_x4,ren_x4[2:0]} = 4'b1_001;
  { `FPM_FMAD,3'b01_0}:{inc2_x4,ren_x4[2:0]} = 4'b1_000;
  { `FPM_FMAD,3'b01_1}:{inc2_x4,ren_x4[2:0]} = 4'b1_010;
  { `FPM_FMAD,3'b10_0}:{inc2_x4,ren_x4[2:0]} = 4'b1_010;
  { `FPM_FMAD,3'b10_1}:{inc2_x4,ren_x4[2:0]} = 4'b1_000;
  { `FPM_FMAD,3'b11_?}:{inc2_x4,ren_x4[2:0]} = 4'b1_000;
  { `FPM_IMUL,3'b??_?}:{inc2_x4,ren_x4[2:0]} = 4'b0_000;
  default             :{inc2_x4,ren_x4[2:0]} = 4'bx;
  endcase
assign infsel_x4 = (rm_x4[1:0]==2'b00)
                 | (rm_x4[1:0]==2'b01) &  rsgn_x4
                 | (rm_x4[1:0]==2'b10) & ~rsgn_x4;
endmodule
module fpmoa
(  sgnf, asel, tf, passQNAN, IE, DF, fpop, fmad, eanz, eano, manz, ebnz,
   ebno, mbnz, ecnz, ecno, mcnz, sgna, sgnb, sgnc, mams, mbms, mcms,
   RM1
);
output       sgnf;
output       asel;
output [2:0] tf;
output       passQNAN;
output       IE;
output       DF;
input        fpop;
input        fmad;
input        eanz;
input        eano;
input        manz;
input        ebnz;
input        ebno;
input        mbnz;
input        ecnz;
input        ecno;
input        mcnz;
input        sgna;
input        sgnb;
input        sgnc;
input        mams;
input        mbms;
input        mcms;
input        RM1;
wire DF;
wire sgnf;
wire asel;
wire passQNAN;
wire IE;
wire [2:0] tf;
   assign DF = mcnz & ~ecnz & fmad
             | mbnz & ~ebnz & fpop
             | manz & ~eanz & fpop;
   assign sgnf = RM1 & sgnc & ebno & eano & fmad
               | ecnz & sgnc & ebno & eano & fmad
               | RM1 & ecno & ebno & ~sgnb & eano & sgna & fmad
               | sgnc & ebno & ~sgnb & eano & sgna & fmad
               | ecno & ebnz & ~sgnb & eanz & sgna & fmad
               | sgnc & ebnz & ~sgnb & eanz & sgna & fmad
               | RM1 & ecno & ebno & sgnb & eano & ~sgna & fmad
               | sgnc & ebno & sgnb & eano & ~sgna & fmad
               | ecno & ebnz & sgnb & eanz & ~sgna & fmad
               | sgnc & ebnz & sgnb & eanz & ~sgna & fmad
               | ebno & ~sgnb & eano & sgna & ~fmad & fpop
               | ebnz & ~sgnb & eanz & sgna & ~fmad & fpop
               | ebno & sgnb & eano & ~sgna & ~fmad & fpop
               | ebnz & sgnb & eanz & ~sgna & ~fmad & fpop;
   assign asel = ecno & ebno & ebnz & eano & eanz & fmad
               | ebno & ebnz & eano & eanz & ~fmad & fpop;
   assign tf[2] = mcnz & ~ecno & fmad
                | ecno & ecnz & ~ebnz & eano & fmad
                | ecno & ecnz & ebno & ~eanz & fmad
                | mbnz & ~ebno & fpop
                | manz & ~eano & fpop;
   assign tf[1] = ~mcms & mcnz & ~ecno & mbms & mams & fmad
                | ~mcms & mcnz & ~ecno & ~mbnz & mams & fmad
                | ~mcms & mcnz & ~ecno & ebno & mams & fmad
                | ~mcms & mcnz & ~ecno & mbms & ~manz & fmad
                | ~mcms & mcnz & ~ecno & mbms & eano & fmad
                | ~ecno & ebno & eano & fmad
                | ecnz & ~ebnz & eano & fmad
                | ecnz & ebno & ~eanz & fmad
                | ~mbnz & ~ebno & eano & fpop
                | ~mbnz & ~manz & ~eano & fpop
                | ebno & ~manz & ~eano & fpop;
   assign tf[0] = ~( ecno & ebno & ebnz
                   | ebno & manz
                   | ebno & eano
                   | mbms & manz & ~eano
                   | ~mbnz & manz & ~eano
                   | ~mams & manz & ~eano
                   | ecno & ~mbnz & ~ebno & eanz
                   | ~sgnc & ebno & ebnz & sgnb & sgna
                   | sgnc & ebno & ebnz & ~sgnb & sgna
                   | ~sgnc & ~mbnz & ~ebno & sgnb & eanz & sgna
                   | sgnc & ~mbnz & ~ebno & ~sgnb & eanz & sgna
                   | sgnc & ebno & ebnz & sgnb & ~sgna
                   | ~sgnc & ebno & ebnz & ~sgnb & ~sgna
                   | sgnc & ~mbnz & ~ebno & sgnb & eanz & ~sgna
                   | ~sgnc & ~mbnz & ~ebno & ~sgnb & eanz & ~sgna
                   | ~mcms & mcnz & ~ecno & mbms & fmad
                   | mcnz & ~ecno & ~mbnz & fmad
                   | mcnz & ~ecno & ebno & fmad
                   | ebno & ebnz & ~fmad
                   | ~mbnz & ~ebno & eanz & ~fmad
                   | ~fpop
                   );
   assign passQNAN = ~( ecno & ebno & eano);
   assign IE = ~mcms & mcnz & ~ecno & fmad
             | ~mcnz & ~ebnz & ~manz & ~eano & fmad
             | ecno & ~ebnz & ~manz & ~eano & fmad
             | ~mcnz & ~mbnz & ~ebno & ~eanz & fmad
             | ecno & ~mbnz & ~ebno & ~eanz & fmad
             | ~mcnz & ~ecno & sgnc & ~mbnz & ~ebno & sgnb & eano & sgna & fmad
             | ~mcnz & ~ecno & ~sgnc & ~mbnz & ~ebno & ~sgnb & eano & sgna & fmad
             | ~mcnz & ~ecno & sgnc & ~mbnz & sgnb & ~manz & ~eano & sgna & fmad
             | ~mcnz & ~ecno & sgnc & ebno & sgnb & ~manz & ~eano & sgna & fmad
             | ~mcnz & ~ecno & ~sgnc & ~mbnz & ~sgnb & ~manz & ~eano & sgna & fmad
             | ~mcnz & ~ecno & ~sgnc & ebno & ~sgnb & ~manz & ~eano & sgna & fmad
             | ~mcnz & ~ecno & ~sgnc & ~mbnz & ~ebno & sgnb & eano & ~sgna & fmad
             | ~mcnz & ~ecno & sgnc & ~mbnz & ~ebno & ~sgnb & eano & ~sgna & fmad
             | ~mcnz & ~ecno & ~sgnc & ~mbnz & sgnb & ~manz & ~eano & ~sgna & fmad
             | ~mcnz & ~ecno & ~sgnc & ebno & sgnb & ~manz & ~eano & ~sgna & fmad
             | ~mcnz & ~ecno & sgnc & ~mbnz & ~sgnb & ~manz & ~eano & ~sgna & fmad
             | ~mcnz & ~ecno & sgnc & ebno & ~sgnb & ~manz & ~eano & ~sgna & fmad
             | ~mbms & mbnz & ~ebno & fpop
             | ~mams & manz & ~eano & fpop
             | ~ebnz & ~manz & ~eano & ~fmad & fpop
             | ~mbnz & ~ebno & ~eanz & ~fmad & fpop;
endmodule
module fpmdp1 (
  sgna_x1, sgnb_x1, sgnc_x1, eanz_x1, ebnz_x1, ecnz_x1, eano_x1, ebno_x1, ecno_x1, manz_x1, mbnz_x1,
  mcnz_x1, mams_x1, mbms_x1, mcms_x1, rsgn_x4, imul_x4, nmr_x4, nr_x4, sadj_x4, e2m1_x4, e2p0_x4, e2p1_x4,
  ovfm1_x4, ovfp0_x4, ovfp1_x4, unfnm1_x4, unfnp0_x4, unfnp1_x4, ezm1_x4, ezp0_x4, mskm1_x4,
  mskp0_x4, mskp1_x4, mrz_x4,
  SrcA_x1, SrcB_x1, SrcC_x1, mad_x1, ppop_x1, tf_x2, sgnf_x2, passQNAN_x2, rm1_x3, fpop_x3, fpop_x4, infsel_x4
);
output         sgna_x1;
output         sgnb_x1;
output         sgnc_x1;
output         eanz_x1;
output         ebnz_x1;
output         ecnz_x1;
output         eano_x1;
output         ebno_x1;
output         ecno_x1;
output         manz_x1;
output         mbnz_x1;
output         mcnz_x1;
output         mams_x1;
output         mbms_x1;
output         mcms_x1;
output         rsgn_x4;
output [63:0]  imul_x4;
output [33:0]  nmr_x4;
output [31:0]  nr_x4;
output         sadj_x4;
output  [7:0]  e2m1_x4;
output  [7:0]  e2p0_x4;
output  [7:0]  e2p1_x4;
output         ovfm1_x4;
output         ovfp0_x4;
output         ovfp1_x4;
output         unfnm1_x4;
output         unfnp0_x4;
output         unfnp1_x4;
output         ezm1_x4;
output         ezp0_x4;
output  [3:0]  mskm1_x4;
output  [3:0]  mskp0_x4;
output  [3:0]  mskp1_x4;
output         mrz_x4;
input   [31:0] SrcA_x1;
input   [31:0] SrcB_x1;
input   [31:0] SrcC_x1;
input          mad_x1;
input    [2:0] ppop_x1;
input    [2:0] tf_x2;
input          sgnf_x2;
input          passQNAN_x2;
input          rm1_x3;
input          fpop_x3;
input          fpop_x4;
input          infsel_x4;
wire [6:0] dsc_x1;
wire       dscunf_x1;
wire [33:0] p00_x1;
wire [33:0] p01_x1;
wire [33:0] p02_x1;
wire [33:0] p03_x1;
wire [33:0] p04_x1;
wire [33:0] p05_x1;
wire [33:0] p06_x1;
wire [33:0] p07_x1;
wire [33:0] p08_x1;
wire [33:0] p09_x1;
wire [33:0] p10_x1;
wire [33:0] p11_x1;
wire [33:0] p12_x1;
wire [33:0] p13_x1;
wire [33:0] p14_x1;
wire [33:0] p15_x1;
wire [33:0] p16_x1;
wire        neg00_x1;
wire        neg01_x1;
wire        neg02_x1;
wire        neg03_x1;
wire        neg04_x1;
wire        neg05_x1;
wire        neg06_x1;
wire        neg07_x1;
wire        neg08_x1;
wire        neg09_x1;
wire        neg10_x1;
wire        neg11_x1;
wire        neg12_x1;
wire        neg13_x1;
wire        neg14_x1;
wire        neg15_x1;
wire [44:00] s20_x1;
wire         c20_00_x1;
wire         c20_03_x1;
wire [39:05] c20_x1;
wire [50:07] s21_x1;
wire [50:11] c21_x1;
wire         s22_16_x1;
wire [57:18] s22_x1;
wire [57:20] c22_x1;
wire         s23_24_x1;
wire [65:26] s23_x1;
wire [65:28] c23_x1;
wire         eanz_x1;
wire         ebnz_x1;
wire         ecnz_x1;
wire         eano_x1;
wire         ebno_x1;
wire         ecno_x1;
wire         manz_x1;
wire         mbnz_x1;
wire         mcnz_x1;
wire         msgn_x1;
wire         csgn_x1;
wire         sub_x1;
wire   [1:0] dshc_x1;
wire  [31:0] SrcA_x2;
wire  [31:0] SrcB_x2;
wire  [31:0] SrcC_x2;
wire   [6:0] dsc_x2;
wire         dscunf_x2;
wire [44:00] s20_x2;
wire         c20_00_x2;
wire         c20_03_x2;
wire [39:05] c20_x2;
wire [50:07] s21_x2;
wire [50:11] c21_x2;
wire         s22_16_x2;
wire [57:18] s22_x2;
wire [57:20] c22_x2;
wire         s23_24_x2;
wire [65:26] s23_x2;
wire [65:28] c23_x2;
wire         msgn_x2;
wire         csgn_x2;
wire         sub_x2;
wire   [1:0] dshc_x2;
wire  [74:0] opc_x2;
wire [74:00] ms_x2;
wire [75:01] mc_x2;
wire   [9:0] e2_x2;
wire [31:0] forced_x2;
wire [31:0] pass_x2;
wire [31:0] nr_x2;
wire [74:00] ms_x3;
wire [75:01] mc_x3;
wire  [31:0] nr_x3;
wire   [9:0] e2_x3;
wire         msgn_x3;
wire         csgn_x3;
wire         sub_x3;
wire [74:0] dmr_x3;
wire        dmrg_x3;
wire        dmrp_x3;
wire  [6:0] nsc_x3;
wire  [9:0] e2i_x3;
wire        mrz_x3;
wire        rsgn_x3;
wire [74:0] dmr_x4;
wire [31:0] nr_x4;
wire        mrz_x4;
wire  [6:0] nsc_x4;
wire  [9:0] e2_x4;
wire  [9:0] e2i_x4;
wire        rsgn_x4;
wire [33:0] nmr_x4;
wire        stki_x4;
wire        sadj_x4;
wire [63:0] imul_x4;
wire  [7:0] e2m1_x4;
wire  [7:0] e2p0_x4;
wire  [7:0] e2p1_x4;
wire        ovfm1_x4;
wire        ovfp0_x4;
wire        ovfp1_x4;
wire        unfnm1_x4;
wire        unfnp0_x4;
wire        unfnp1_x4;
wire        ezm1_x4;
wire        ezp0_x4;
wire  [3:0] mskm1_x4;
wire  [3:0] mskp0_x4;
wire  [3:0] mskp1_x4;
wire  [9:8] unused_e2m1_x4;
wire  [9:8] unused_e2p0_x4;
wire  [9:8] unused_e2p1_x4;
fpmed fpmed (
   .dsc    (dsc_x1[6:0]),
   .dscunf (dscunf_x1),
   .mad    (mad_x1),
   .ea     (SrcA_x1[30:23]),
   .eb     (SrcB_x1[30:23]),
   .ec     (SrcC_x1[30:23])
);
fpmpp fpmpp (
   .p00   (p00_x1[33:0]),
   .p01   (p01_x1[33:0]),
   .p02   (p02_x1[33:0]),
   .p03   (p03_x1[33:0]),
   .p04   (p04_x1[33:0]),
   .p05   (p05_x1[33:0]),
   .p06   (p06_x1[33:0]),
   .p07   (p07_x1[33:0]),
   .p08   (p08_x1[33:0]),
   .p09   (p09_x1[33:0]),
   .p10   (p10_x1[33:0]),
   .p11   (p11_x1[33:0]),
   .p12   (p12_x1[33:0]),
   .p13   (p13_x1[33:0]),
   .p14   (p14_x1[33:0]),
   .p15   (p15_x1[33:0]),
   .p16   (p16_x1[33:0]),
   .neg00 (neg00_x1),
   .neg01 (neg01_x1),
   .neg02 (neg02_x1),
   .neg03 (neg03_x1),
   .neg04 (neg04_x1),
   .neg05 (neg05_x1),
   .neg06 (neg06_x1),
   .neg07 (neg07_x1),
   .neg08 (neg08_x1),
   .neg09 (neg09_x1),
   .neg10 (neg10_x1),
   .neg11 (neg11_x1),
   .neg12 (neg12_x1),
   .neg13 (neg13_x1),
   .neg14 (neg14_x1),
   .neg15 (neg15_x1),
   .opa   (SrcA_x1[31:0]),
   .opb   (SrcB_x1[31:0]),
   .op    (ppop_x1[2:0])
);
fpmcp1 fpmcp1 (
   .s20    (s20_x1[44:00]),
   .c20_00 (c20_00_x1),
   .c20_03 (c20_03_x1),
   .c20    (c20_x1[39:05]),
   .s21    (s21_x1[50:07]),
   .c21    (c21_x1[50:11]),
   .s22_16 (s22_16_x1),
   .s22    (s22_x1[57:18]),
   .c22    (c22_x1[57:20]),
   .s23_24 (s23_24_x1),
   .s23    (s23_x1[65:26]),
   .c23    (c23_x1[65:28]),
   .p00    (p00_x1[33:0]),
   .p01    (p01_x1[33:0]),
   .p02    (p02_x1[33:0]),
   .p03    (p03_x1[33:0]),
   .p04    (p04_x1[33:0]),
   .p05    (p05_x1[33:0]),
   .p06    (p06_x1[33:0]),
   .p07    (p07_x1[33:0]),
   .p08    (p08_x1[33:0]),
   .p09    (p09_x1[33:0]),
   .p10    (p10_x1[33:0]),
   .p11    (p11_x1[33:0]),
   .p12    (p12_x1[33:0]),
   .p13    (p13_x1[33:0]),
   .p14    (p14_x1[33:0]),
   .p15    (p15_x1[33:0]),
   .p16    (p16_x1[33:0]),
   .neg00  (neg00_x1),
   .neg01  (neg01_x1),
   .neg02  (neg02_x1),
   .neg03  (neg03_x1),
   .neg04  (neg04_x1),
   .neg05  (neg05_x1),
   .neg06  (neg06_x1),
   .neg07  (neg07_x1),
   .neg08  (neg08_x1),
   .neg09  (neg09_x1),
   .neg10  (neg10_x1),
   .neg11  (neg11_x1),
   .neg12  (neg12_x1),
   .neg13  (neg13_x1),
   .neg14  (neg14_x1),
   .neg15  (neg15_x1)
);
fpmot fpmot (
   .sgna (sgna_x1),
   .sgnb (sgnb_x1),
   .sgnc (sgnc_x1),
   .eanz (eanz_x1),
   .ebnz (ebnz_x1),
   .ecnz (ecnz_x1),
   .eano (eano_x1),
   .ebno (ebno_x1),
   .ecno (ecno_x1),
   .manz (manz_x1),
   .mbnz (mbnz_x1),
   .mcnz (mcnz_x1),
   .mams (mams_x1),
   .mbms (mbms_x1),
   .mcms (mcms_x1),
   .msgn (msgn_x1),
   .csgn (csgn_x1),
   .dshc (dshc_x1[1:0]),
   .sub  (sub_x1),
   .mad  (mad_x1),
   .SrcA (SrcA_x1[31:0]),
   .SrcB (SrcB_x1[31:0]),
   .SrcC (SrcC_x1[31:0])
);
assign s20_x2[44:00] = s20_x1[44:00];
assign c20_00_x2     = c20_00_x1;
assign c20_03_x2     = c20_03_x1;
assign c20_x2[39:05] = c20_x1[39:05];
assign s21_x2[50:07] = s21_x1[50:07];
assign c21_x2[50:11] = c21_x1[50:11];
assign s22_16_x2     = s22_16_x1;
assign s22_x2[57:18] = s22_x1[57:18];
assign c22_x2[57:20] = c22_x1[57:20];
assign s23_24_x2     = s23_24_x1;
assign s23_x2[65:26] = s23_x1[65:26];
assign c23_x2[65:28] = c23_x1[65:28];
assign SrcA_x2[31:0] = SrcA_x1[31:0];
assign SrcB_x2[31:0] = SrcB_x1[31:0];
assign SrcC_x2[31:0] = SrcC_x1[31:0];
assign sub_x2        = sub_x1;
assign dshc_x2[1:0]  = dshc_x1[1:0];
assign dsc_x2[6:0]   = dsc_x1[6:0];
assign dscunf_x2     = dscunf_x1;
assign csgn_x2       = csgn_x1;
assign msgn_x2       = msgn_x1;
fpmdsh fpmdsh (
   .opc   (opc_x2[74:1]),
   .mc    ({1'b1,SrcC_x2[22:0]}),
   .dsc   (dsc_x2[6:0]),
   .dshc  (dshc_x2[1:0])
);
fpmdsk fpmdsk (
   .stky  (opc_x2[0]),
   .mc    ({1'b1,SrcC_x2[22:0]}),
   .dsc   (dsc_x2[6:0]),
   .dshc  (dshc_x2[1:0])
);
fpmcp2 fpmcp2 (
   .ms     (ms_x2[74:00]),
   .mc     (mc_x2[75:01]),
   .s20    (s20_x2[44:00]),
   .c20_00 (c20_00_x2),
   .c20_03 (c20_03_x2),
   .c20    (c20_x2[39:05]),
   .s21    (s21_x2[50:07]),
   .c21    (c21_x2[50:11]),
   .s22_16 (s22_16_x2),
   .s22    (s22_x2[57:18]),
   .c22    (c22_x2[57:20]),
   .s23_24 (s23_24_x2),
   .s23    (s23_x2[65:26]),
   .c23    (c23_x2[65:28]),
   .opc    (opc_x2[74:00])
);
fpme2 fpme2 (
   .e2     (e2_x2[9:0]),
   .dscunf (dscunf_x2),
   .SrcA   (SrcA_x2[30:23]),
   .SrcB   (SrcB_x2[30:23]),
   .SrcC   (SrcC_x2[30:23])
);
fp_mux3 #(32) mx_pass_x2 (
   .y  (pass_x2[31:0]),
   .i0 (SrcA_x2[31:0]),
   .i1 (SrcB_x2[31:0]),
   .i2 (SrcC_x2[31:0]),
   .s  (tf_x2[1:0])
);
assign
   forced_x2[31:0] = {sgnf_x2, {8{tf_x2[1]}}, tf_x2[0], 22'b0 },
   nr_x2[31:0] = tf_x2[2] ? {pass_x2[31:23], pass_x2[22] | passQNAN_x2, pass_x2[21:0]}
                          : forced_x2[31:0];
assign nr_x3[31:0] =  nr_x2[31:0];
assign ms_x3[74:0] =  ms_x2[74:0];
assign mc_x3[75:1] =  mc_x2[75:1];
assign sub_x3      =  sub_x2;
assign e2_x3[9:0]  =  e2_x2[9:0];
assign msgn_x3     =  msgn_x2;
assign csgn_x3     =  csgn_x2;
fpmadd fpmadd (
   .dmr (dmr_x3[74:0]),
   .g   (dmrg_x3),
   .p   (dmrp_x3),
   .ms  (ms_x3[74:0]),
   .mc  (mc_x3[75:1]),
   .sub (sub_x3)
);
fpmlzp fpmlzp (
   .nsc  (nsc_x3[6:0]),
   .ms   (ms_x3[74:0]),
   .mc   (mc_x3[75:1]),
   .co   (dmrg_x3),
   .sub  (sub_x3),
   .fpop (fpop_x3)
);
fpme3 fpme3 (
   .e2i    (e2i_x3[9:0]),
   .mrz    (mrz_x3),
   .rsgn   (rsgn_x3),
   .e2     (e2_x3[9:0]),
   .sub    (sub_x3),
   .dmrg   (dmrg_x3),
   .dmrp   (dmrp_x3),
   .msgn   (msgn_x3),
   .csgn   (csgn_x3),
   .rm1    (rm1_x3)
);
assign nr_x4[31:0]  = nr_x3[31:0];
assign dmr_x4[74:0] = dmr_x3[74:0];
assign nsc_x4[6:0]  = nsc_x3[6:0];
assign mrz_x4       = mrz_x3;
assign e2_x4[9:0]   = e2_x3[9:0];
assign e2i_x4[9:0]  = e2i_x3[9:0];
assign rsgn_x4      = rsgn_x3;
fpmnsk fpmnsk (
   .stky (stki_x4),
   .dmr  (dmr_x4[49:0]),
   .nsc  (nsc_x4[6:0])
);
fpmnsh fpmnsh (
   .nmr  (nmr_x4[33:0]),
   .sadj (sadj_x4),
   .dmr  (dmr_x4[74:0]),
   .nsc  (nsc_x4[6:0]),
   .stki (stki_x4),
   .fpop (fpop_x4)
);
assign imul_x4 = dmr_x4[64:1];
fpme4 fpme4 (
   .e2m1     ({unused_e2m1_x4[9:8], e2m1_x4[7:0]}),
   .e2p0     ({unused_e2p0_x4[9:8], e2p0_x4[7:0]}),
   .e2p1     ({unused_e2p1_x4[9:8], e2p1_x4[7:0]}),
   .ovfm1    (ovfm1_x4),
   .ovfp0    (ovfp0_x4),
   .ovfp1    (ovfp1_x4),
   .unfnm1   (unfnm1_x4),
   .unfnp0   (unfnp0_x4),
   .unfnp1   (unfnp1_x4),
   .ezm1     (ezm1_x4),
   .ezp0     (ezp0_x4),
   .mskm1    (mskm1_x4[3:0]),
   .mskp0    (mskp0_x4[3:0]),
   .mskp1    (mskp1_x4[3:0]),
   .e2       (e2_x4[9:0]),
   .e2i      (e2i_x4[9:0]),
   .nsc      (nsc_x4[6:0]),
   .mrz      (mrz_x4),
   .infsel   (infsel_x4)
);
endmodule
module fpmed (dsc, dscunf, mad, ea, eb, ec);
output [6:0] dsc;
output       dscunf;
input        mad;
input  [7:0] ea;
input  [7:0] eb;
input  [7:0] ec;
wire [7:0] diffs;
wire [8:1] diffc;
wire [8:1] ds;
wire [8:1] dc;
wire       ge0;
wire [8:0] cnt;
wire [8:2] ms;
wire [8:2] mc;
wire       ge74;
fp_add3 U0 (.co(diffc[1]), .s(diffs[0]), .a(ea[0]), .b(eb[0]), .c(~ec[0]));
fp_add3 U1 (.co(diffc[2]), .s(diffs[1]), .a(ea[1]), .b(eb[1]), .c(~ec[1]));
fp_add3 U2 (.co(diffc[3]), .s(diffs[2]), .a(ea[2]), .b(eb[2]), .c(~ec[2]));
fp_add3 U3 (.co(diffc[4]), .s(diffs[3]), .a(ea[3]), .b(eb[3]), .c(~ec[3]));
fp_add3 U4 (.co(diffc[5]), .s(diffs[4]), .a(ea[4]), .b(eb[4]), .c(~ec[4]));
fp_add3 U5 (.co(diffc[6]), .s(diffs[5]), .a(ea[5]), .b(eb[5]), .c(~ec[5]));
fp_add3 U6 (.co(diffc[7]), .s(diffs[6]), .a(ea[6]), .b(eb[6]), .c(~ec[6]));
fp_add3 U7 (.co(diffc[8]), .s(diffs[7]), .a(ea[7]), .b(eb[7]), .c(~ec[7]));
assign cnt[0] = ~(           diffs[0]);
assign ds[1]  =  (diffc[1] ^ diffs[1]);
assign ds[2]  = ~(diffc[2] ^ diffs[2]);
assign ds[3]  = ~(diffc[3] ^ diffs[3]);
assign ds[4]  = ~(diffc[4] ^ diffs[4]);
assign ds[5]  =  (diffc[5] ^ diffs[5]);
assign ds[6]  =  (diffc[6] ^ diffs[6]);
assign ds[7]  = ~(diffc[7] ^ diffs[7]);
assign ds[8]  =  (diffc[8]           );
assign dc[1] =  (           diffs[0]);
assign dc[2] =  (diffc[1] & diffs[1]);
assign dc[3] =  (diffc[2] | diffs[2]);
assign dc[4] =  (diffc[3] | diffs[3]);
assign dc[5] =  (diffc[4] | diffs[4]);
assign dc[6] =  (diffc[5] & diffs[5]);
assign dc[7] =  (diffc[6] & diffs[6]);
assign dc[8] =  (diffc[7] | diffs[7]);
fpadd8o fpadd8o_cnt (.co(ge0), .sum(cnt[8:1]), .a(ds[8:1]), .b(dc[8:1]));
assign dscunf = ~ge0 & mad;
assign ms[2] =  (diffc[2] ^ diffs[2]);
assign ms[3] =  (diffc[3] ^ diffs[3]);
assign ms[4] = ~(diffc[4] ^ diffs[4]);
assign ms[5] =  (diffc[5] ^ diffs[5]);
assign ms[6] = ~(diffc[6] ^ diffs[6]);
assign ms[7] =  (diffc[7] ^ diffs[7]);
assign ms[8] =  (diffc[8]           ) ;
assign mc[2] =  (diffc[1] | diffs[1]);
assign mc[3] =  (diffc[2] & diffs[2]);
assign mc[4] =  (diffc[3] & diffs[3]);
assign mc[5] =  (diffc[4] | diffs[4]);
assign mc[6] =  (diffc[5] & diffs[5]);
assign mc[7] =  (diffc[6] | diffs[6]);
assign mc[8] =  (diffc[7] & diffs[7]);
fpcry7 fpcry7_ge74 (.co(ge74), .a(mc[8:2]), .b(ms[8:2]));
assign dsc[6:0] = ge74 ? 7'd74 : {7{ge0}} & cnt[6:0];
wire [8:7] unused_cnt = cnt[8:7];
endmodule
module fpmot (
   sgna, sgnb, sgnc, eanz, ebnz, ecnz, eano, ebno,
   ecno, manz, mbnz, mcnz, mams, mbms, mcms, msgn,
   csgn, dshc, sub,
   mad, SrcA, SrcB, SrcC
);
output        sgna;
output        sgnb;
output        sgnc;
output        eanz;
output        ebnz;
output        ecnz;
output        eano;
output        ebno;
output        ecno;
output        manz;
output        mbnz;
output        mcnz;
output        mams;
output        mbms;
output        mcms;
output        msgn;
output        csgn;
output  [1:0] dshc;
output        sub;
input         mad;
input  [31:0] SrcA;
input  [31:0] SrcB;
input  [31:0] SrcC;
assign
   sgna    = SrcA[31],
   sgnb    = SrcB[31],
   sgnc    = SrcC[31],
   eanz    =  (|SrcA[30:23]),
   ebnz    =  (|SrcB[30:23]),
   ecnz    =  (|SrcC[30:23]),
   eano    = ~(&SrcA[30:23]),
   ebno    = ~(&SrcB[30:23]),
   ecno    = ~(&SrcC[30:23]),
   manz    =  (|SrcA[22:0]),
   mbnz    =  (|SrcB[22:0]),
   mcnz    =  (|SrcC[22:0]),
   mams    = SrcA[22],
   mbms    = SrcB[22],
   mcms    = SrcC[22],
   msgn    = SrcA[31] ^ SrcB[31],
   csgn    = SrcC[31] & ecnz & mad,
   dshc[0] = ~(msgn ^ SrcC[31]) & ecnz & mad,
   dshc[1] =  (msgn ^ SrcC[31]) & ecnz & mad,
   sub     = dshc[1];
endmodule
module fpmpp (
   p00, p01, p02, p03, p04, p05, p06, p07,
   p08, p09, p10, p11, p12, p13, p14, p15, p16,
   neg00, neg01, neg02, neg03, neg04, neg05, neg06, neg07,
   neg08, neg09, neg10, neg11, neg12, neg13, neg14, neg15,
   opa, opb, op
);
output [33:0] p00;
output [33:0] p01;
output [33:0] p02;
output [33:0] p03;
output [33:0] p04;
output [33:0] p05;
output [33:0] p06;
output [33:0] p07;
output [33:0] p08;
output [33:0] p09;
output [33:0] p10;
output [33:0] p11;
output [33:0] p12;
output [33:0] p13;
output [33:0] p14;
output [33:0] p15;
output [33:0] p16;
output        neg00;
output        neg01;
output        neg02;
output        neg03;
output        neg04;
output        neg05;
output        neg06;
output        neg07;
output        neg08;
output        neg09;
output        neg10;
output        neg11;
output        neg12;
output        neg13;
output        neg14;
output        neg15;
input  [31:0] opa;
input  [31:0] opb;
input   [2:0] op;
wire [32:23] opx;
wire [32:23] opy;
wire   [2:0] sel00;
wire   [2:0] sel01;
wire   [2:0] sel02;
wire   [2:0] sel03;
wire   [2:0] sel04;
wire   [2:0] sel05;
wire   [2:0] sel06;
wire   [2:0] sel07;
wire   [2:0] sel08;
wire   [2:0] sel09;
wire   [2:0] sel10;
wire   [2:0] sel11;
wire   [2:0] sel12;
wire   [2:0] sel13;
wire   [2:0] sel14;
wire   [2:0] sel15;
wire   [2:0] sel16;
assign opx[32:23] = {op[1] & opa[31], {8{op[2]}} & opa[31:24], ~op[2] | opa[23]};
assign opy[32:23] = {op[0] & opb[31], {8{op[2]}} & opb[31:24], ~op[2] | opb[23]};
fp_benc U00000 (.s(sel00[2]), .a(sel00[1]), .x2(sel00[0]), .m2(opb[01]), .m1(opb[00]), .m0(1'b0) );
fp_benc U00001 (.s(sel01[2]), .a(sel01[1]), .x2(sel01[0]), .m2(opb[03]), .m1(opb[02]), .m0(opb[01]) );
fp_benc U00002 (.s(sel02[2]), .a(sel02[1]), .x2(sel02[0]), .m2(opb[05]), .m1(opb[04]), .m0(opb[03]) );
fp_benc U00003 (.s(sel03[2]), .a(sel03[1]), .x2(sel03[0]), .m2(opb[07]), .m1(opb[06]), .m0(opb[05]) );
fp_benc U00004 (.s(sel04[2]), .a(sel04[1]), .x2(sel04[0]), .m2(opb[09]), .m1(opb[08]), .m0(opb[07]) );
fp_benc U00005 (.s(sel05[2]), .a(sel05[1]), .x2(sel05[0]), .m2(opb[11]), .m1(opb[10]), .m0(opb[09]) );
fp_benc U00006 (.s(sel06[2]), .a(sel06[1]), .x2(sel06[0]), .m2(opb[13]), .m1(opb[12]), .m0(opb[11]) );
fp_benc U00007 (.s(sel07[2]), .a(sel07[1]), .x2(sel07[0]), .m2(opb[15]), .m1(opb[14]), .m0(opb[13]) );
fp_benc U00008 (.s(sel08[2]), .a(sel08[1]), .x2(sel08[0]), .m2(opb[17]), .m1(opb[16]), .m0(opb[15]) );
fp_benc U00009 (.s(sel09[2]), .a(sel09[1]), .x2(sel09[0]), .m2(opb[19]), .m1(opb[18]), .m0(opb[17]) );
fp_benc U00010 (.s(sel10[2]), .a(sel10[1]), .x2(sel10[0]), .m2(opb[21]), .m1(opb[20]), .m0(opb[19]) );
fp_benc U00011 (.s(sel11[2]), .a(sel11[1]), .x2(sel11[0]), .m2(opy[23]), .m1(opb[22]), .m0(opb[21]) );
fp_benc U00012 (.s(sel12[2]), .a(sel12[1]), .x2(sel12[0]), .m2(opy[25]), .m1(opy[24]), .m0(opy[23]) );
fp_benc U00013 (.s(sel13[2]), .a(sel13[1]), .x2(sel13[0]), .m2(opy[27]), .m1(opy[26]), .m0(opy[25]) );
fp_benc U00014 (.s(sel14[2]), .a(sel14[1]), .x2(sel14[0]), .m2(opy[29]), .m1(opy[28]), .m0(opy[27]) );
fp_benc U00015 (.s(sel15[2]), .a(sel15[1]), .x2(sel15[0]), .m2(opy[31]), .m1(opy[30]), .m0(opy[29]) );
fp_benc U00016 (.s(sel16[2]), .a(sel16[1]), .x2(sel16[0]), .m2(opy[32]), .m1(opy[32]), .m0(opy[31]) );
fp_bmx  U02000 (.pp(p00[00]), .m1(opa[00]), .m0(1'b0),  .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02001 (.pp(p00[01]), .m1(opa[01]), .m0(opa[00]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02002 (.pp(p00[02]), .m1(opa[02]), .m0(opa[01]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02003 (.pp(p00[03]), .m1(opa[03]), .m0(opa[02]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02004 (.pp(p00[04]), .m1(opa[04]), .m0(opa[03]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02005 (.pp(p00[05]), .m1(opa[05]), .m0(opa[04]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02006 (.pp(p00[06]), .m1(opa[06]), .m0(opa[05]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02007 (.pp(p00[07]), .m1(opa[07]), .m0(opa[06]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02008 (.pp(p00[08]), .m1(opa[08]), .m0(opa[07]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02009 (.pp(p00[09]), .m1(opa[09]), .m0(opa[08]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02010 (.pp(p00[10]), .m1(opa[10]), .m0(opa[09]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02011 (.pp(p00[11]), .m1(opa[11]), .m0(opa[10]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02012 (.pp(p00[12]), .m1(opa[12]), .m0(opa[11]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02013 (.pp(p00[13]), .m1(opa[13]), .m0(opa[12]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02014 (.pp(p00[14]), .m1(opa[14]), .m0(opa[13]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02015 (.pp(p00[15]), .m1(opa[15]), .m0(opa[14]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02016 (.pp(p00[16]), .m1(opa[16]), .m0(opa[15]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02017 (.pp(p00[17]), .m1(opa[17]), .m0(opa[16]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02018 (.pp(p00[18]), .m1(opa[18]), .m0(opa[17]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02019 (.pp(p00[19]), .m1(opa[19]), .m0(opa[18]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02020 (.pp(p00[20]), .m1(opa[20]), .m0(opa[19]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02021 (.pp(p00[21]), .m1(opa[21]), .m0(opa[20]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02022 (.pp(p00[22]), .m1(opa[22]), .m0(opa[21]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02023 (.pp(p00[23]), .m1(opx[23]), .m0(opa[22]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02024 (.pp(p00[24]), .m1(opx[24]), .m0(opx[23]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02025 (.pp(p00[25]), .m1(opx[25]), .m0(opx[24]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02026 (.pp(p00[26]), .m1(opx[26]), .m0(opx[25]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02027 (.pp(p00[27]), .m1(opx[27]), .m0(opx[26]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02028 (.pp(p00[28]), .m1(opx[28]), .m0(opx[27]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02029 (.pp(p00[29]), .m1(opx[29]), .m0(opx[28]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02030 (.pp(p00[30]), .m1(opx[30]), .m0(opx[29]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02031 (.pp(p00[31]), .m1(opx[31]), .m0(opx[30]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02032 (.pp(p00[32]), .m1(opx[32]), .m0(opx[31]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_bmx  U02033 (.pp(p00[33]), .m1(opx[32]), .m0(opx[32]), .s(sel00[2]), .a(sel00[1]), .x2(sel00[0]) );
fp_not  U03000 (.y(neg00), .a(sel00[2]) );
fp_bmx  U03002 (.pp(p01[00]), .m1(opa[00]), .m0(1'b0),  .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03003 (.pp(p01[01]), .m1(opa[01]), .m0(opa[00]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03004 (.pp(p01[02]), .m1(opa[02]), .m0(opa[01]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03005 (.pp(p01[03]), .m1(opa[03]), .m0(opa[02]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03006 (.pp(p01[04]), .m1(opa[04]), .m0(opa[03]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03007 (.pp(p01[05]), .m1(opa[05]), .m0(opa[04]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03008 (.pp(p01[06]), .m1(opa[06]), .m0(opa[05]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03009 (.pp(p01[07]), .m1(opa[07]), .m0(opa[06]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03010 (.pp(p01[08]), .m1(opa[08]), .m0(opa[07]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03011 (.pp(p01[09]), .m1(opa[09]), .m0(opa[08]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03012 (.pp(p01[10]), .m1(opa[10]), .m0(opa[09]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03013 (.pp(p01[11]), .m1(opa[11]), .m0(opa[10]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03014 (.pp(p01[12]), .m1(opa[12]), .m0(opa[11]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03015 (.pp(p01[13]), .m1(opa[13]), .m0(opa[12]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03016 (.pp(p01[14]), .m1(opa[14]), .m0(opa[13]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03017 (.pp(p01[15]), .m1(opa[15]), .m0(opa[14]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03018 (.pp(p01[16]), .m1(opa[16]), .m0(opa[15]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03019 (.pp(p01[17]), .m1(opa[17]), .m0(opa[16]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03020 (.pp(p01[18]), .m1(opa[18]), .m0(opa[17]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03021 (.pp(p01[19]), .m1(opa[19]), .m0(opa[18]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03022 (.pp(p01[20]), .m1(opa[20]), .m0(opa[19]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03023 (.pp(p01[21]), .m1(opa[21]), .m0(opa[20]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03024 (.pp(p01[22]), .m1(opa[22]), .m0(opa[21]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03025 (.pp(p01[23]), .m1(opx[23]), .m0(opa[22]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03026 (.pp(p01[24]), .m1(opx[24]), .m0(opx[23]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03027 (.pp(p01[25]), .m1(opx[25]), .m0(opx[24]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03028 (.pp(p01[26]), .m1(opx[26]), .m0(opx[25]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03029 (.pp(p01[27]), .m1(opx[27]), .m0(opx[26]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03030 (.pp(p01[28]), .m1(opx[28]), .m0(opx[27]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03031 (.pp(p01[29]), .m1(opx[29]), .m0(opx[28]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03032 (.pp(p01[30]), .m1(opx[30]), .m0(opx[29]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03033 (.pp(p01[31]), .m1(opx[31]), .m0(opx[30]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03034 (.pp(p01[32]), .m1(opx[32]), .m0(opx[31]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_bmx  U03035 (.pp(p01[33]), .m1(opx[32]), .m0(opx[32]), .s(sel01[2]), .a(sel01[1]), .x2(sel01[0]) );
fp_not  U04002 (.y(neg01), .a(sel01[2]) );
fp_bmx  U04004 (.pp(p02[00]), .m1(opa[00]), .m0(1'b0),  .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04005 (.pp(p02[01]), .m1(opa[01]), .m0(opa[00]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04006 (.pp(p02[02]), .m1(opa[02]), .m0(opa[01]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04007 (.pp(p02[03]), .m1(opa[03]), .m0(opa[02]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04008 (.pp(p02[04]), .m1(opa[04]), .m0(opa[03]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04009 (.pp(p02[05]), .m1(opa[05]), .m0(opa[04]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04010 (.pp(p02[06]), .m1(opa[06]), .m0(opa[05]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04011 (.pp(p02[07]), .m1(opa[07]), .m0(opa[06]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04012 (.pp(p02[08]), .m1(opa[08]), .m0(opa[07]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04013 (.pp(p02[09]), .m1(opa[09]), .m0(opa[08]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04014 (.pp(p02[10]), .m1(opa[10]), .m0(opa[09]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04015 (.pp(p02[11]), .m1(opa[11]), .m0(opa[10]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04016 (.pp(p02[12]), .m1(opa[12]), .m0(opa[11]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04017 (.pp(p02[13]), .m1(opa[13]), .m0(opa[12]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04018 (.pp(p02[14]), .m1(opa[14]), .m0(opa[13]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04019 (.pp(p02[15]), .m1(opa[15]), .m0(opa[14]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04020 (.pp(p02[16]), .m1(opa[16]), .m0(opa[15]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04021 (.pp(p02[17]), .m1(opa[17]), .m0(opa[16]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04022 (.pp(p02[18]), .m1(opa[18]), .m0(opa[17]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04023 (.pp(p02[19]), .m1(opa[19]), .m0(opa[18]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04024 (.pp(p02[20]), .m1(opa[20]), .m0(opa[19]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04025 (.pp(p02[21]), .m1(opa[21]), .m0(opa[20]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04026 (.pp(p02[22]), .m1(opa[22]), .m0(opa[21]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04027 (.pp(p02[23]), .m1(opx[23]), .m0(opa[22]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04028 (.pp(p02[24]), .m1(opx[24]), .m0(opx[23]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04029 (.pp(p02[25]), .m1(opx[25]), .m0(opx[24]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04030 (.pp(p02[26]), .m1(opx[26]), .m0(opx[25]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04031 (.pp(p02[27]), .m1(opx[27]), .m0(opx[26]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04032 (.pp(p02[28]), .m1(opx[28]), .m0(opx[27]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04033 (.pp(p02[29]), .m1(opx[29]), .m0(opx[28]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04034 (.pp(p02[30]), .m1(opx[30]), .m0(opx[29]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04035 (.pp(p02[31]), .m1(opx[31]), .m0(opx[30]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04036 (.pp(p02[32]), .m1(opx[32]), .m0(opx[31]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_bmx  U04037 (.pp(p02[33]), .m1(opx[32]), .m0(opx[32]), .s(sel02[2]), .a(sel02[1]), .x2(sel02[0]) );
fp_not  U05004 (.y(neg02), .a(sel02[2]) );
fp_bmx  U05006 (.pp(p03[00]), .m1(opa[00]), .m0(1'b0),  .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05007 (.pp(p03[01]), .m1(opa[01]), .m0(opa[00]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05008 (.pp(p03[02]), .m1(opa[02]), .m0(opa[01]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05009 (.pp(p03[03]), .m1(opa[03]), .m0(opa[02]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05010 (.pp(p03[04]), .m1(opa[04]), .m0(opa[03]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05011 (.pp(p03[05]), .m1(opa[05]), .m0(opa[04]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05012 (.pp(p03[06]), .m1(opa[06]), .m0(opa[05]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05013 (.pp(p03[07]), .m1(opa[07]), .m0(opa[06]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05014 (.pp(p03[08]), .m1(opa[08]), .m0(opa[07]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05015 (.pp(p03[09]), .m1(opa[09]), .m0(opa[08]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05016 (.pp(p03[10]), .m1(opa[10]), .m0(opa[09]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05017 (.pp(p03[11]), .m1(opa[11]), .m0(opa[10]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05018 (.pp(p03[12]), .m1(opa[12]), .m0(opa[11]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05019 (.pp(p03[13]), .m1(opa[13]), .m0(opa[12]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05020 (.pp(p03[14]), .m1(opa[14]), .m0(opa[13]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05021 (.pp(p03[15]), .m1(opa[15]), .m0(opa[14]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05022 (.pp(p03[16]), .m1(opa[16]), .m0(opa[15]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05023 (.pp(p03[17]), .m1(opa[17]), .m0(opa[16]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05024 (.pp(p03[18]), .m1(opa[18]), .m0(opa[17]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05025 (.pp(p03[19]), .m1(opa[19]), .m0(opa[18]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05026 (.pp(p03[20]), .m1(opa[20]), .m0(opa[19]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05027 (.pp(p03[21]), .m1(opa[21]), .m0(opa[20]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05028 (.pp(p03[22]), .m1(opa[22]), .m0(opa[21]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05029 (.pp(p03[23]), .m1(opx[23]), .m0(opa[22]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05030 (.pp(p03[24]), .m1(opx[24]), .m0(opx[23]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05031 (.pp(p03[25]), .m1(opx[25]), .m0(opx[24]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05032 (.pp(p03[26]), .m1(opx[26]), .m0(opx[25]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05033 (.pp(p03[27]), .m1(opx[27]), .m0(opx[26]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05034 (.pp(p03[28]), .m1(opx[28]), .m0(opx[27]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05035 (.pp(p03[29]), .m1(opx[29]), .m0(opx[28]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05036 (.pp(p03[30]), .m1(opx[30]), .m0(opx[29]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05037 (.pp(p03[31]), .m1(opx[31]), .m0(opx[30]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05038 (.pp(p03[32]), .m1(opx[32]), .m0(opx[31]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_bmx  U05039 (.pp(p03[33]), .m1(opx[32]), .m0(opx[32]), .s(sel03[2]), .a(sel03[1]), .x2(sel03[0]) );
fp_not  U06006 (.y(neg03), .a(sel03[2]) );
fp_bmx  U06008 (.pp(p04[00]), .m1(opa[00]), .m0(1'b0),  .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06009 (.pp(p04[01]), .m1(opa[01]), .m0(opa[00]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06010 (.pp(p04[02]), .m1(opa[02]), .m0(opa[01]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06011 (.pp(p04[03]), .m1(opa[03]), .m0(opa[02]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06012 (.pp(p04[04]), .m1(opa[04]), .m0(opa[03]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06013 (.pp(p04[05]), .m1(opa[05]), .m0(opa[04]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06014 (.pp(p04[06]), .m1(opa[06]), .m0(opa[05]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06015 (.pp(p04[07]), .m1(opa[07]), .m0(opa[06]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06016 (.pp(p04[08]), .m1(opa[08]), .m0(opa[07]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06017 (.pp(p04[09]), .m1(opa[09]), .m0(opa[08]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06018 (.pp(p04[10]), .m1(opa[10]), .m0(opa[09]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06019 (.pp(p04[11]), .m1(opa[11]), .m0(opa[10]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06020 (.pp(p04[12]), .m1(opa[12]), .m0(opa[11]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06021 (.pp(p04[13]), .m1(opa[13]), .m0(opa[12]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06022 (.pp(p04[14]), .m1(opa[14]), .m0(opa[13]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06023 (.pp(p04[15]), .m1(opa[15]), .m0(opa[14]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06024 (.pp(p04[16]), .m1(opa[16]), .m0(opa[15]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06025 (.pp(p04[17]), .m1(opa[17]), .m0(opa[16]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06026 (.pp(p04[18]), .m1(opa[18]), .m0(opa[17]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06027 (.pp(p04[19]), .m1(opa[19]), .m0(opa[18]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06028 (.pp(p04[20]), .m1(opa[20]), .m0(opa[19]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06029 (.pp(p04[21]), .m1(opa[21]), .m0(opa[20]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06030 (.pp(p04[22]), .m1(opa[22]), .m0(opa[21]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06031 (.pp(p04[23]), .m1(opx[23]), .m0(opa[22]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06032 (.pp(p04[24]), .m1(opx[24]), .m0(opx[23]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06033 (.pp(p04[25]), .m1(opx[25]), .m0(opx[24]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06034 (.pp(p04[26]), .m1(opx[26]), .m0(opx[25]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06035 (.pp(p04[27]), .m1(opx[27]), .m0(opx[26]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06036 (.pp(p04[28]), .m1(opx[28]), .m0(opx[27]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06037 (.pp(p04[29]), .m1(opx[29]), .m0(opx[28]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06038 (.pp(p04[30]), .m1(opx[30]), .m0(opx[29]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06039 (.pp(p04[31]), .m1(opx[31]), .m0(opx[30]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06040 (.pp(p04[32]), .m1(opx[32]), .m0(opx[31]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_bmx  U06041 (.pp(p04[33]), .m1(opx[32]), .m0(opx[32]), .s(sel04[2]), .a(sel04[1]), .x2(sel04[0]) );
fp_not  U07008 (.y(neg04), .a(sel04[2]) );
fp_bmx  U07010 (.pp(p05[00]), .m1(opa[00]), .m0(1'b0),  .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07011 (.pp(p05[01]), .m1(opa[01]), .m0(opa[00]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07012 (.pp(p05[02]), .m1(opa[02]), .m0(opa[01]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07013 (.pp(p05[03]), .m1(opa[03]), .m0(opa[02]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07014 (.pp(p05[04]), .m1(opa[04]), .m0(opa[03]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07015 (.pp(p05[05]), .m1(opa[05]), .m0(opa[04]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07016 (.pp(p05[06]), .m1(opa[06]), .m0(opa[05]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07017 (.pp(p05[07]), .m1(opa[07]), .m0(opa[06]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07018 (.pp(p05[08]), .m1(opa[08]), .m0(opa[07]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07019 (.pp(p05[09]), .m1(opa[09]), .m0(opa[08]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07020 (.pp(p05[10]), .m1(opa[10]), .m0(opa[09]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07021 (.pp(p05[11]), .m1(opa[11]), .m0(opa[10]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07022 (.pp(p05[12]), .m1(opa[12]), .m0(opa[11]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07023 (.pp(p05[13]), .m1(opa[13]), .m0(opa[12]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07024 (.pp(p05[14]), .m1(opa[14]), .m0(opa[13]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07025 (.pp(p05[15]), .m1(opa[15]), .m0(opa[14]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07026 (.pp(p05[16]), .m1(opa[16]), .m0(opa[15]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07027 (.pp(p05[17]), .m1(opa[17]), .m0(opa[16]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07028 (.pp(p05[18]), .m1(opa[18]), .m0(opa[17]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07029 (.pp(p05[19]), .m1(opa[19]), .m0(opa[18]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07030 (.pp(p05[20]), .m1(opa[20]), .m0(opa[19]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07031 (.pp(p05[21]), .m1(opa[21]), .m0(opa[20]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07032 (.pp(p05[22]), .m1(opa[22]), .m0(opa[21]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07033 (.pp(p05[23]), .m1(opx[23]), .m0(opa[22]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07034 (.pp(p05[24]), .m1(opx[24]), .m0(opx[23]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07035 (.pp(p05[25]), .m1(opx[25]), .m0(opx[24]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07036 (.pp(p05[26]), .m1(opx[26]), .m0(opx[25]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07037 (.pp(p05[27]), .m1(opx[27]), .m0(opx[26]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07038 (.pp(p05[28]), .m1(opx[28]), .m0(opx[27]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07039 (.pp(p05[29]), .m1(opx[29]), .m0(opx[28]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07040 (.pp(p05[30]), .m1(opx[30]), .m0(opx[29]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07041 (.pp(p05[31]), .m1(opx[31]), .m0(opx[30]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07042 (.pp(p05[32]), .m1(opx[32]), .m0(opx[31]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_bmx  U07043 (.pp(p05[33]), .m1(opx[32]), .m0(opx[32]), .s(sel05[2]), .a(sel05[1]), .x2(sel05[0]) );
fp_not  U08010 (.y(neg05), .a(sel05[2]));
fp_bmx  U08012 (.pp(p06[00]), .m1(opa[00]), .m0(1'b0),  .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08013 (.pp(p06[01]), .m1(opa[01]), .m0(opa[00]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08014 (.pp(p06[02]), .m1(opa[02]), .m0(opa[01]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08015 (.pp(p06[03]), .m1(opa[03]), .m0(opa[02]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08016 (.pp(p06[04]), .m1(opa[04]), .m0(opa[03]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08017 (.pp(p06[05]), .m1(opa[05]), .m0(opa[04]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08018 (.pp(p06[06]), .m1(opa[06]), .m0(opa[05]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08019 (.pp(p06[07]), .m1(opa[07]), .m0(opa[06]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08020 (.pp(p06[08]), .m1(opa[08]), .m0(opa[07]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08021 (.pp(p06[09]), .m1(opa[09]), .m0(opa[08]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08022 (.pp(p06[10]), .m1(opa[10]), .m0(opa[09]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08023 (.pp(p06[11]), .m1(opa[11]), .m0(opa[10]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08024 (.pp(p06[12]), .m1(opa[12]), .m0(opa[11]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08025 (.pp(p06[13]), .m1(opa[13]), .m0(opa[12]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08026 (.pp(p06[14]), .m1(opa[14]), .m0(opa[13]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08027 (.pp(p06[15]), .m1(opa[15]), .m0(opa[14]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08028 (.pp(p06[16]), .m1(opa[16]), .m0(opa[15]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08029 (.pp(p06[17]), .m1(opa[17]), .m0(opa[16]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08030 (.pp(p06[18]), .m1(opa[18]), .m0(opa[17]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08031 (.pp(p06[19]), .m1(opa[19]), .m0(opa[18]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08032 (.pp(p06[20]), .m1(opa[20]), .m0(opa[19]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08033 (.pp(p06[21]), .m1(opa[21]), .m0(opa[20]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08034 (.pp(p06[22]), .m1(opa[22]), .m0(opa[21]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08035 (.pp(p06[23]), .m1(opx[23]), .m0(opa[22]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08036 (.pp(p06[24]), .m1(opx[24]), .m0(opx[23]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08037 (.pp(p06[25]), .m1(opx[25]), .m0(opx[24]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08038 (.pp(p06[26]), .m1(opx[26]), .m0(opx[25]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08039 (.pp(p06[27]), .m1(opx[27]), .m0(opx[26]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08040 (.pp(p06[28]), .m1(opx[28]), .m0(opx[27]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08041 (.pp(p06[29]), .m1(opx[29]), .m0(opx[28]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08042 (.pp(p06[30]), .m1(opx[30]), .m0(opx[29]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08043 (.pp(p06[31]), .m1(opx[31]), .m0(opx[30]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08044 (.pp(p06[32]), .m1(opx[32]), .m0(opx[31]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_bmx  U08045 (.pp(p06[33]), .m1(opx[32]), .m0(opx[32]), .s(sel06[2]), .a(sel06[1]), .x2(sel06[0]) );
fp_not  U09012 (.y(neg06), .a(sel06[2]));
fp_bmx  U09014 (.pp(p07[00]), .m1(opa[00]), .m0(1'b0),  .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09015 (.pp(p07[01]), .m1(opa[01]), .m0(opa[00]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09016 (.pp(p07[02]), .m1(opa[02]), .m0(opa[01]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09017 (.pp(p07[03]), .m1(opa[03]), .m0(opa[02]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09018 (.pp(p07[04]), .m1(opa[04]), .m0(opa[03]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09019 (.pp(p07[05]), .m1(opa[05]), .m0(opa[04]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09020 (.pp(p07[06]), .m1(opa[06]), .m0(opa[05]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09021 (.pp(p07[07]), .m1(opa[07]), .m0(opa[06]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09022 (.pp(p07[08]), .m1(opa[08]), .m0(opa[07]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09023 (.pp(p07[09]), .m1(opa[09]), .m0(opa[08]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09024 (.pp(p07[10]), .m1(opa[10]), .m0(opa[09]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09025 (.pp(p07[11]), .m1(opa[11]), .m0(opa[10]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09026 (.pp(p07[12]), .m1(opa[12]), .m0(opa[11]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09027 (.pp(p07[13]), .m1(opa[13]), .m0(opa[12]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09028 (.pp(p07[14]), .m1(opa[14]), .m0(opa[13]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09029 (.pp(p07[15]), .m1(opa[15]), .m0(opa[14]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09030 (.pp(p07[16]), .m1(opa[16]), .m0(opa[15]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09031 (.pp(p07[17]), .m1(opa[17]), .m0(opa[16]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09032 (.pp(p07[18]), .m1(opa[18]), .m0(opa[17]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09033 (.pp(p07[19]), .m1(opa[19]), .m0(opa[18]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09034 (.pp(p07[20]), .m1(opa[20]), .m0(opa[19]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09035 (.pp(p07[21]), .m1(opa[21]), .m0(opa[20]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09036 (.pp(p07[22]), .m1(opa[22]), .m0(opa[21]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09037 (.pp(p07[23]), .m1(opx[23]), .m0(opa[22]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09038 (.pp(p07[24]), .m1(opx[24]), .m0(opx[23]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09039 (.pp(p07[25]), .m1(opx[25]), .m0(opx[24]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09040 (.pp(p07[26]), .m1(opx[26]), .m0(opx[25]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09041 (.pp(p07[27]), .m1(opx[27]), .m0(opx[26]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09042 (.pp(p07[28]), .m1(opx[28]), .m0(opx[27]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09043 (.pp(p07[29]), .m1(opx[29]), .m0(opx[28]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09044 (.pp(p07[30]), .m1(opx[30]), .m0(opx[29]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09045 (.pp(p07[31]), .m1(opx[31]), .m0(opx[30]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09046 (.pp(p07[32]), .m1(opx[32]), .m0(opx[31]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_bmx  U09047 (.pp(p07[33]), .m1(opx[32]), .m0(opx[32]), .s(sel07[2]), .a(sel07[1]), .x2(sel07[0]) );
fp_not  U10014 (.y(neg07), .a(sel07[2]));
fp_bmx  U10016 (.pp(p08[00]), .m1(opa[00]), .m0(1'b0),  .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10017 (.pp(p08[01]), .m1(opa[01]), .m0(opa[00]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10018 (.pp(p08[02]), .m1(opa[02]), .m0(opa[01]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10019 (.pp(p08[03]), .m1(opa[03]), .m0(opa[02]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10020 (.pp(p08[04]), .m1(opa[04]), .m0(opa[03]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10021 (.pp(p08[05]), .m1(opa[05]), .m0(opa[04]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10022 (.pp(p08[06]), .m1(opa[06]), .m0(opa[05]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10023 (.pp(p08[07]), .m1(opa[07]), .m0(opa[06]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10024 (.pp(p08[08]), .m1(opa[08]), .m0(opa[07]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10025 (.pp(p08[09]), .m1(opa[09]), .m0(opa[08]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10026 (.pp(p08[10]), .m1(opa[10]), .m0(opa[09]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10027 (.pp(p08[11]), .m1(opa[11]), .m0(opa[10]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10028 (.pp(p08[12]), .m1(opa[12]), .m0(opa[11]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10029 (.pp(p08[13]), .m1(opa[13]), .m0(opa[12]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10030 (.pp(p08[14]), .m1(opa[14]), .m0(opa[13]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10031 (.pp(p08[15]), .m1(opa[15]), .m0(opa[14]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10032 (.pp(p08[16]), .m1(opa[16]), .m0(opa[15]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10033 (.pp(p08[17]), .m1(opa[17]), .m0(opa[16]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10034 (.pp(p08[18]), .m1(opa[18]), .m0(opa[17]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10035 (.pp(p08[19]), .m1(opa[19]), .m0(opa[18]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10036 (.pp(p08[20]), .m1(opa[20]), .m0(opa[19]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10037 (.pp(p08[21]), .m1(opa[21]), .m0(opa[20]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10038 (.pp(p08[22]), .m1(opa[22]), .m0(opa[21]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10039 (.pp(p08[23]), .m1(opx[23]), .m0(opa[22]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10040 (.pp(p08[24]), .m1(opx[24]), .m0(opx[23]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10041 (.pp(p08[25]), .m1(opx[25]), .m0(opx[24]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10042 (.pp(p08[26]), .m1(opx[26]), .m0(opx[25]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10043 (.pp(p08[27]), .m1(opx[27]), .m0(opx[26]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10044 (.pp(p08[28]), .m1(opx[28]), .m0(opx[27]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10045 (.pp(p08[29]), .m1(opx[29]), .m0(opx[28]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10046 (.pp(p08[30]), .m1(opx[30]), .m0(opx[29]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10047 (.pp(p08[31]), .m1(opx[31]), .m0(opx[30]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10048 (.pp(p08[32]), .m1(opx[32]), .m0(opx[31]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_bmx  U10049 (.pp(p08[33]), .m1(opx[32]), .m0(opx[32]), .s(sel08[2]), .a(sel08[1]), .x2(sel08[0]) );
fp_not  U11016 (.y(neg08), .a(sel08[2]));
fp_bmx  U11018 (.pp(p09[00]), .m1(opa[00]), .m0(1'b0),  .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11019 (.pp(p09[01]), .m1(opa[01]), .m0(opa[00]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11020 (.pp(p09[02]), .m1(opa[02]), .m0(opa[01]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11021 (.pp(p09[03]), .m1(opa[03]), .m0(opa[02]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11022 (.pp(p09[04]), .m1(opa[04]), .m0(opa[03]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11023 (.pp(p09[05]), .m1(opa[05]), .m0(opa[04]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11024 (.pp(p09[06]), .m1(opa[06]), .m0(opa[05]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11025 (.pp(p09[07]), .m1(opa[07]), .m0(opa[06]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11026 (.pp(p09[08]), .m1(opa[08]), .m0(opa[07]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11027 (.pp(p09[09]), .m1(opa[09]), .m0(opa[08]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11028 (.pp(p09[10]), .m1(opa[10]), .m0(opa[09]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11029 (.pp(p09[11]), .m1(opa[11]), .m0(opa[10]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11030 (.pp(p09[12]), .m1(opa[12]), .m0(opa[11]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11031 (.pp(p09[13]), .m1(opa[13]), .m0(opa[12]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11032 (.pp(p09[14]), .m1(opa[14]), .m0(opa[13]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11033 (.pp(p09[15]), .m1(opa[15]), .m0(opa[14]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11034 (.pp(p09[16]), .m1(opa[16]), .m0(opa[15]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11035 (.pp(p09[17]), .m1(opa[17]), .m0(opa[16]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11036 (.pp(p09[18]), .m1(opa[18]), .m0(opa[17]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11037 (.pp(p09[19]), .m1(opa[19]), .m0(opa[18]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11038 (.pp(p09[20]), .m1(opa[20]), .m0(opa[19]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11039 (.pp(p09[21]), .m1(opa[21]), .m0(opa[20]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11040 (.pp(p09[22]), .m1(opa[22]), .m0(opa[21]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11041 (.pp(p09[23]), .m1(opx[23]), .m0(opa[22]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11042 (.pp(p09[24]), .m1(opx[24]), .m0(opx[23]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11043 (.pp(p09[25]), .m1(opx[25]), .m0(opx[24]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11044 (.pp(p09[26]), .m1(opx[26]), .m0(opx[25]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11045 (.pp(p09[27]), .m1(opx[27]), .m0(opx[26]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11046 (.pp(p09[28]), .m1(opx[28]), .m0(opx[27]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11047 (.pp(p09[29]), .m1(opx[29]), .m0(opx[28]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11048 (.pp(p09[30]), .m1(opx[30]), .m0(opx[29]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11049 (.pp(p09[31]), .m1(opx[31]), .m0(opx[30]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11050 (.pp(p09[32]), .m1(opx[32]), .m0(opx[31]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_bmx  U11051 (.pp(p09[33]), .m1(opx[32]), .m0(opx[32]), .s(sel09[2]), .a(sel09[1]), .x2(sel09[0]) );
fp_not  U12016 (.y(neg09), .a(sel09[2]));
fp_bmx  U12018 (.pp(p10[00]), .m1(opa[00]), .m0(1'b0),  .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12019 (.pp(p10[01]), .m1(opa[01]), .m0(opa[00]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12020 (.pp(p10[02]), .m1(opa[02]), .m0(opa[01]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12021 (.pp(p10[03]), .m1(opa[03]), .m0(opa[02]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12022 (.pp(p10[04]), .m1(opa[04]), .m0(opa[03]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12023 (.pp(p10[05]), .m1(opa[05]), .m0(opa[04]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12024 (.pp(p10[06]), .m1(opa[06]), .m0(opa[05]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12025 (.pp(p10[07]), .m1(opa[07]), .m0(opa[06]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12026 (.pp(p10[08]), .m1(opa[08]), .m0(opa[07]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12027 (.pp(p10[09]), .m1(opa[09]), .m0(opa[08]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12028 (.pp(p10[10]), .m1(opa[10]), .m0(opa[09]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12029 (.pp(p10[11]), .m1(opa[11]), .m0(opa[10]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12030 (.pp(p10[12]), .m1(opa[12]), .m0(opa[11]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12031 (.pp(p10[13]), .m1(opa[13]), .m0(opa[12]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12032 (.pp(p10[14]), .m1(opa[14]), .m0(opa[13]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12033 (.pp(p10[15]), .m1(opa[15]), .m0(opa[14]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12034 (.pp(p10[16]), .m1(opa[16]), .m0(opa[15]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12035 (.pp(p10[17]), .m1(opa[17]), .m0(opa[16]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12036 (.pp(p10[18]), .m1(opa[18]), .m0(opa[17]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12037 (.pp(p10[19]), .m1(opa[19]), .m0(opa[18]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12038 (.pp(p10[20]), .m1(opa[20]), .m0(opa[19]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12039 (.pp(p10[21]), .m1(opa[21]), .m0(opa[20]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12040 (.pp(p10[22]), .m1(opa[22]), .m0(opa[21]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12041 (.pp(p10[23]), .m1(opx[23]), .m0(opa[22]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12042 (.pp(p10[24]), .m1(opx[24]), .m0(opx[23]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12043 (.pp(p10[25]), .m1(opx[25]), .m0(opx[24]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12044 (.pp(p10[26]), .m1(opx[26]), .m0(opx[25]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12045 (.pp(p10[27]), .m1(opx[27]), .m0(opx[26]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12046 (.pp(p10[28]), .m1(opx[28]), .m0(opx[27]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12047 (.pp(p10[29]), .m1(opx[29]), .m0(opx[28]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12048 (.pp(p10[30]), .m1(opx[30]), .m0(opx[29]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12049 (.pp(p10[31]), .m1(opx[31]), .m0(opx[30]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12050 (.pp(p10[32]), .m1(opx[32]), .m0(opx[31]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_bmx  U12051 (.pp(p10[33]), .m1(opx[32]), .m0(opx[32]), .s(sel10[2]), .a(sel10[1]), .x2(sel10[0]) );
fp_not  U13016 (.y(neg10), .a(sel10[2]));
fp_bmx  U13018 (.pp(p11[00]), .m1(opa[00]), .m0(1'b0),  .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13019 (.pp(p11[01]), .m1(opa[01]), .m0(opa[00]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13020 (.pp(p11[02]), .m1(opa[02]), .m0(opa[01]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13021 (.pp(p11[03]), .m1(opa[03]), .m0(opa[02]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13022 (.pp(p11[04]), .m1(opa[04]), .m0(opa[03]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13023 (.pp(p11[05]), .m1(opa[05]), .m0(opa[04]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13024 (.pp(p11[06]), .m1(opa[06]), .m0(opa[05]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13025 (.pp(p11[07]), .m1(opa[07]), .m0(opa[06]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13026 (.pp(p11[08]), .m1(opa[08]), .m0(opa[07]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13027 (.pp(p11[09]), .m1(opa[09]), .m0(opa[08]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13028 (.pp(p11[10]), .m1(opa[10]), .m0(opa[09]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13029 (.pp(p11[11]), .m1(opa[11]), .m0(opa[10]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13030 (.pp(p11[12]), .m1(opa[12]), .m0(opa[11]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13031 (.pp(p11[13]), .m1(opa[13]), .m0(opa[12]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13032 (.pp(p11[14]), .m1(opa[14]), .m0(opa[13]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13033 (.pp(p11[15]), .m1(opa[15]), .m0(opa[14]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13034 (.pp(p11[16]), .m1(opa[16]), .m0(opa[15]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13035 (.pp(p11[17]), .m1(opa[17]), .m0(opa[16]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13036 (.pp(p11[18]), .m1(opa[18]), .m0(opa[17]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13037 (.pp(p11[19]), .m1(opa[19]), .m0(opa[18]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13038 (.pp(p11[20]), .m1(opa[20]), .m0(opa[19]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13039 (.pp(p11[21]), .m1(opa[21]), .m0(opa[20]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13040 (.pp(p11[22]), .m1(opa[22]), .m0(opa[21]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13041 (.pp(p11[23]), .m1(opx[23]), .m0(opa[22]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13042 (.pp(p11[24]), .m1(opx[24]), .m0(opx[23]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13043 (.pp(p11[25]), .m1(opx[25]), .m0(opx[24]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13044 (.pp(p11[26]), .m1(opx[26]), .m0(opx[25]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13045 (.pp(p11[27]), .m1(opx[27]), .m0(opx[26]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13046 (.pp(p11[28]), .m1(opx[28]), .m0(opx[27]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13047 (.pp(p11[29]), .m1(opx[29]), .m0(opx[28]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13048 (.pp(p11[30]), .m1(opx[30]), .m0(opx[29]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13049 (.pp(p11[31]), .m1(opx[31]), .m0(opx[30]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13050 (.pp(p11[32]), .m1(opx[32]), .m0(opx[31]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_bmx  U13051 (.pp(p11[33]), .m1(opx[32]), .m0(opx[32]), .s(sel11[2]), .a(sel11[1]), .x2(sel11[0]) );
fp_not  U14016 (.y(neg11), .a(sel11[2]));
fp_bmx  U14018 (.pp(p12[00]), .m1(opa[00]), .m0(1'b0),  .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14019 (.pp(p12[01]), .m1(opa[01]), .m0(opa[00]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14020 (.pp(p12[02]), .m1(opa[02]), .m0(opa[01]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14021 (.pp(p12[03]), .m1(opa[03]), .m0(opa[02]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14022 (.pp(p12[04]), .m1(opa[04]), .m0(opa[03]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14023 (.pp(p12[05]), .m1(opa[05]), .m0(opa[04]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14024 (.pp(p12[06]), .m1(opa[06]), .m0(opa[05]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14025 (.pp(p12[07]), .m1(opa[07]), .m0(opa[06]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14026 (.pp(p12[08]), .m1(opa[08]), .m0(opa[07]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14027 (.pp(p12[09]), .m1(opa[09]), .m0(opa[08]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14028 (.pp(p12[10]), .m1(opa[10]), .m0(opa[09]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14029 (.pp(p12[11]), .m1(opa[11]), .m0(opa[10]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14030 (.pp(p12[12]), .m1(opa[12]), .m0(opa[11]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14031 (.pp(p12[13]), .m1(opa[13]), .m0(opa[12]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14032 (.pp(p12[14]), .m1(opa[14]), .m0(opa[13]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14033 (.pp(p12[15]), .m1(opa[15]), .m0(opa[14]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14034 (.pp(p12[16]), .m1(opa[16]), .m0(opa[15]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14035 (.pp(p12[17]), .m1(opa[17]), .m0(opa[16]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14036 (.pp(p12[18]), .m1(opa[18]), .m0(opa[17]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14037 (.pp(p12[19]), .m1(opa[19]), .m0(opa[18]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14038 (.pp(p12[20]), .m1(opa[20]), .m0(opa[19]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14039 (.pp(p12[21]), .m1(opa[21]), .m0(opa[20]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14040 (.pp(p12[22]), .m1(opa[22]), .m0(opa[21]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14041 (.pp(p12[23]), .m1(opx[23]), .m0(opa[22]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14042 (.pp(p12[24]), .m1(opx[24]), .m0(opx[23]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14043 (.pp(p12[25]), .m1(opx[25]), .m0(opx[24]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14044 (.pp(p12[26]), .m1(opx[26]), .m0(opx[25]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14045 (.pp(p12[27]), .m1(opx[27]), .m0(opx[26]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14046 (.pp(p12[28]), .m1(opx[28]), .m0(opx[27]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14047 (.pp(p12[29]), .m1(opx[29]), .m0(opx[28]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14048 (.pp(p12[30]), .m1(opx[30]), .m0(opx[29]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14049 (.pp(p12[31]), .m1(opx[31]), .m0(opx[30]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14050 (.pp(p12[32]), .m1(opx[32]), .m0(opx[31]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_bmx  U14051 (.pp(p12[33]), .m1(opx[32]), .m0(opx[32]), .s(sel12[2]), .a(sel12[1]), .x2(sel12[0]) );
fp_not  U15016 (.y(neg12), .a(sel12[2]));
fp_bmx  U15018 (.pp(p13[00]), .m1(opa[00]), .m0(1'b0),  .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15019 (.pp(p13[01]), .m1(opa[01]), .m0(opa[00]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15020 (.pp(p13[02]), .m1(opa[02]), .m0(opa[01]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15021 (.pp(p13[03]), .m1(opa[03]), .m0(opa[02]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15022 (.pp(p13[04]), .m1(opa[04]), .m0(opa[03]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15023 (.pp(p13[05]), .m1(opa[05]), .m0(opa[04]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15024 (.pp(p13[06]), .m1(opa[06]), .m0(opa[05]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15025 (.pp(p13[07]), .m1(opa[07]), .m0(opa[06]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15026 (.pp(p13[08]), .m1(opa[08]), .m0(opa[07]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15027 (.pp(p13[09]), .m1(opa[09]), .m0(opa[08]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15028 (.pp(p13[10]), .m1(opa[10]), .m0(opa[09]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15029 (.pp(p13[11]), .m1(opa[11]), .m0(opa[10]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15030 (.pp(p13[12]), .m1(opa[12]), .m0(opa[11]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15031 (.pp(p13[13]), .m1(opa[13]), .m0(opa[12]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15032 (.pp(p13[14]), .m1(opa[14]), .m0(opa[13]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15033 (.pp(p13[15]), .m1(opa[15]), .m0(opa[14]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15034 (.pp(p13[16]), .m1(opa[16]), .m0(opa[15]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15035 (.pp(p13[17]), .m1(opa[17]), .m0(opa[16]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15036 (.pp(p13[18]), .m1(opa[18]), .m0(opa[17]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15037 (.pp(p13[19]), .m1(opa[19]), .m0(opa[18]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15038 (.pp(p13[20]), .m1(opa[20]), .m0(opa[19]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15039 (.pp(p13[21]), .m1(opa[21]), .m0(opa[20]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15040 (.pp(p13[22]), .m1(opa[22]), .m0(opa[21]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15041 (.pp(p13[23]), .m1(opx[23]), .m0(opa[22]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15042 (.pp(p13[24]), .m1(opx[24]), .m0(opx[23]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15043 (.pp(p13[25]), .m1(opx[25]), .m0(opx[24]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15044 (.pp(p13[26]), .m1(opx[26]), .m0(opx[25]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15045 (.pp(p13[27]), .m1(opx[27]), .m0(opx[26]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15046 (.pp(p13[28]), .m1(opx[28]), .m0(opx[27]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15047 (.pp(p13[29]), .m1(opx[29]), .m0(opx[28]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15048 (.pp(p13[30]), .m1(opx[30]), .m0(opx[29]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15049 (.pp(p13[31]), .m1(opx[31]), .m0(opx[30]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15050 (.pp(p13[32]), .m1(opx[32]), .m0(opx[31]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_bmx  U15051 (.pp(p13[33]), .m1(opx[32]), .m0(opx[32]), .s(sel13[2]), .a(sel13[1]), .x2(sel13[0]) );
fp_not  U16016 (.y(neg13), .a(sel13[2]));
fp_bmx  U16018 (.pp(p14[00]), .m1(opa[00]), .m0(1'b0),  .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16019 (.pp(p14[01]), .m1(opa[01]), .m0(opa[00]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16020 (.pp(p14[02]), .m1(opa[02]), .m0(opa[01]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16021 (.pp(p14[03]), .m1(opa[03]), .m0(opa[02]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16022 (.pp(p14[04]), .m1(opa[04]), .m0(opa[03]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16023 (.pp(p14[05]), .m1(opa[05]), .m0(opa[04]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16024 (.pp(p14[06]), .m1(opa[06]), .m0(opa[05]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16025 (.pp(p14[07]), .m1(opa[07]), .m0(opa[06]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16026 (.pp(p14[08]), .m1(opa[08]), .m0(opa[07]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16027 (.pp(p14[09]), .m1(opa[09]), .m0(opa[08]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16028 (.pp(p14[10]), .m1(opa[10]), .m0(opa[09]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16029 (.pp(p14[11]), .m1(opa[11]), .m0(opa[10]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16030 (.pp(p14[12]), .m1(opa[12]), .m0(opa[11]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16031 (.pp(p14[13]), .m1(opa[13]), .m0(opa[12]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16032 (.pp(p14[14]), .m1(opa[14]), .m0(opa[13]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16033 (.pp(p14[15]), .m1(opa[15]), .m0(opa[14]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16034 (.pp(p14[16]), .m1(opa[16]), .m0(opa[15]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16035 (.pp(p14[17]), .m1(opa[17]), .m0(opa[16]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16036 (.pp(p14[18]), .m1(opa[18]), .m0(opa[17]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16037 (.pp(p14[19]), .m1(opa[19]), .m0(opa[18]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16038 (.pp(p14[20]), .m1(opa[20]), .m0(opa[19]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16039 (.pp(p14[21]), .m1(opa[21]), .m0(opa[20]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16040 (.pp(p14[22]), .m1(opa[22]), .m0(opa[21]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16041 (.pp(p14[23]), .m1(opx[23]), .m0(opa[22]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16042 (.pp(p14[24]), .m1(opx[24]), .m0(opx[23]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16043 (.pp(p14[25]), .m1(opx[25]), .m0(opx[24]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16044 (.pp(p14[26]), .m1(opx[26]), .m0(opx[25]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16045 (.pp(p14[27]), .m1(opx[27]), .m0(opx[26]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16046 (.pp(p14[28]), .m1(opx[28]), .m0(opx[27]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16047 (.pp(p14[29]), .m1(opx[29]), .m0(opx[28]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16048 (.pp(p14[30]), .m1(opx[30]), .m0(opx[29]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16049 (.pp(p14[31]), .m1(opx[31]), .m0(opx[30]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16050 (.pp(p14[32]), .m1(opx[32]), .m0(opx[31]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_bmx  U16051 (.pp(p14[33]), .m1(opx[32]), .m0(opx[32]), .s(sel14[2]), .a(sel14[1]), .x2(sel14[0]) );
fp_not  U17016 (.y(neg14), .a(sel14[2]));
fp_bmx  U17018 (.pp(p15[00]), .m1(opa[00]), .m0(1'b0),  .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17019 (.pp(p15[01]), .m1(opa[01]), .m0(opa[00]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17020 (.pp(p15[02]), .m1(opa[02]), .m0(opa[01]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17021 (.pp(p15[03]), .m1(opa[03]), .m0(opa[02]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17022 (.pp(p15[04]), .m1(opa[04]), .m0(opa[03]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17023 (.pp(p15[05]), .m1(opa[05]), .m0(opa[04]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17024 (.pp(p15[06]), .m1(opa[06]), .m0(opa[05]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17025 (.pp(p15[07]), .m1(opa[07]), .m0(opa[06]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17026 (.pp(p15[08]), .m1(opa[08]), .m0(opa[07]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17027 (.pp(p15[09]), .m1(opa[09]), .m0(opa[08]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17028 (.pp(p15[10]), .m1(opa[10]), .m0(opa[09]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17029 (.pp(p15[11]), .m1(opa[11]), .m0(opa[10]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17030 (.pp(p15[12]), .m1(opa[12]), .m0(opa[11]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17031 (.pp(p15[13]), .m1(opa[13]), .m0(opa[12]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17032 (.pp(p15[14]), .m1(opa[14]), .m0(opa[13]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17033 (.pp(p15[15]), .m1(opa[15]), .m0(opa[14]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17034 (.pp(p15[16]), .m1(opa[16]), .m0(opa[15]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17035 (.pp(p15[17]), .m1(opa[17]), .m0(opa[16]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17036 (.pp(p15[18]), .m1(opa[18]), .m0(opa[17]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17037 (.pp(p15[19]), .m1(opa[19]), .m0(opa[18]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17038 (.pp(p15[20]), .m1(opa[20]), .m0(opa[19]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17039 (.pp(p15[21]), .m1(opa[21]), .m0(opa[20]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17040 (.pp(p15[22]), .m1(opa[22]), .m0(opa[21]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17041 (.pp(p15[23]), .m1(opx[23]), .m0(opa[22]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17042 (.pp(p15[24]), .m1(opx[24]), .m0(opx[23]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17043 (.pp(p15[25]), .m1(opx[25]), .m0(opx[24]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17044 (.pp(p15[26]), .m1(opx[26]), .m0(opx[25]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17045 (.pp(p15[27]), .m1(opx[27]), .m0(opx[26]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17046 (.pp(p15[28]), .m1(opx[28]), .m0(opx[27]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17047 (.pp(p15[29]), .m1(opx[29]), .m0(opx[28]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17048 (.pp(p15[30]), .m1(opx[30]), .m0(opx[29]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17049 (.pp(p15[31]), .m1(opx[31]), .m0(opx[30]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17050 (.pp(p15[32]), .m1(opx[32]), .m0(opx[31]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_bmx  U17051 (.pp(p15[33]), .m1(opx[32]), .m0(opx[32]), .s(sel15[2]), .a(sel15[1]), .x2(sel15[0]) );
fp_not  U18016 (.y(neg15), .a(sel15[2]));
fp_bmx  U18018 (.pp(p16[00]), .m1(opa[00]), .m0(1'b0),  .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18019 (.pp(p16[01]), .m1(opa[01]), .m0(opa[00]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18020 (.pp(p16[02]), .m1(opa[02]), .m0(opa[01]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18021 (.pp(p16[03]), .m1(opa[03]), .m0(opa[02]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18022 (.pp(p16[04]), .m1(opa[04]), .m0(opa[03]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18023 (.pp(p16[05]), .m1(opa[05]), .m0(opa[04]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18024 (.pp(p16[06]), .m1(opa[06]), .m0(opa[05]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18025 (.pp(p16[07]), .m1(opa[07]), .m0(opa[06]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18026 (.pp(p16[08]), .m1(opa[08]), .m0(opa[07]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18027 (.pp(p16[09]), .m1(opa[09]), .m0(opa[08]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18028 (.pp(p16[10]), .m1(opa[10]), .m0(opa[09]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18029 (.pp(p16[11]), .m1(opa[11]), .m0(opa[10]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18030 (.pp(p16[12]), .m1(opa[12]), .m0(opa[11]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18031 (.pp(p16[13]), .m1(opa[13]), .m0(opa[12]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18032 (.pp(p16[14]), .m1(opa[14]), .m0(opa[13]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18033 (.pp(p16[15]), .m1(opa[15]), .m0(opa[14]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18034 (.pp(p16[16]), .m1(opa[16]), .m0(opa[15]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18035 (.pp(p16[17]), .m1(opa[17]), .m0(opa[16]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18036 (.pp(p16[18]), .m1(opa[18]), .m0(opa[17]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18037 (.pp(p16[19]), .m1(opa[19]), .m0(opa[18]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18038 (.pp(p16[20]), .m1(opa[20]), .m0(opa[19]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18039 (.pp(p16[21]), .m1(opa[21]), .m0(opa[20]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18040 (.pp(p16[22]), .m1(opa[22]), .m0(opa[21]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18041 (.pp(p16[23]), .m1(opx[23]), .m0(opa[22]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18042 (.pp(p16[24]), .m1(opx[24]), .m0(opx[23]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18043 (.pp(p16[25]), .m1(opx[25]), .m0(opx[24]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18044 (.pp(p16[26]), .m1(opx[26]), .m0(opx[25]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18045 (.pp(p16[27]), .m1(opx[27]), .m0(opx[26]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18046 (.pp(p16[28]), .m1(opx[28]), .m0(opx[27]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18047 (.pp(p16[29]), .m1(opx[29]), .m0(opx[28]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18048 (.pp(p16[30]), .m1(opx[30]), .m0(opx[29]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18049 (.pp(p16[31]), .m1(opx[31]), .m0(opx[30]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18050 (.pp(p16[32]), .m1(opx[32]), .m0(opx[31]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
fp_bmx  U18051 (.pp(p16[33]), .m1(opx[32]), .m0(opx[32]), .s(sel16[2]), .a(sel16[1]), .x2(sel16[0]) );
endmodule
module fpmcp1 (
   s20, c20_00, c20_03, c20, s21, c21, s22_16, s22, c22, s23_24, s23, c23,
   p00, p01, p02, p03, p04, p05, p06, p07,
   p08, p09, p10, p11, p12, p13, p14, p15, p16,
   neg00, neg01, neg02, neg03, neg04, neg05, neg06, neg07,
   neg08, neg09, neg10, neg11, neg12, neg13, neg14, neg15
);
output [44:00] s20;
output         c20_00;
output         c20_03;
output [39:05] c20;
output [50:07] s21;
output [50:11] c21;
output         s22_16;
output [57:18] s22;
output [57:20] c22;
output         s23_24;
output [65:26] s23;
output [65:28] c23;
input   [33:0] p00;
input   [33:0] p01;
input   [33:0] p02;
input   [33:0] p03;
input   [33:0] p04;
input   [33:0] p05;
input   [33:0] p06;
input   [33:0] p07;
input   [33:0] p08;
input   [33:0] p09;
input   [33:0] p10;
input   [33:0] p11;
input   [33:0] p12;
input   [33:0] p13;
input   [33:0] p14;
input   [33:0] p15;
input   [33:0] p16;
input          neg00;
input          neg01;
input          neg02;
input          neg03;
input          neg04;
input          neg05;
input          neg06;
input          neg07;
input          neg08;
input          neg09;
input          neg10;
input          neg11;
input          neg12;
input          neg13;
input          neg14;
input          neg15;
wire          q00_33;
wire          q01_33;
wire          q02_33;
wire          q03_33;
wire          q04_33;
wire          q05_33;
wire          q06_33;
wire          q07_33;
wire          q08_33;
wire          q09_33;
wire          q10_33;
wire          q11_33;
wire          q12_33;
wire          q13_33;
wire          q14_33;
wire          q15_33;
wire          q16_33;
wire  [38:00] s10;
wire          c10_00;
wire  [37:03] c10;
wire          s11_04;
wire  [44:06] s11;
wire  [43:07] c11;
wire          s12_10;
wire  [50:12] s12;
wire  [49:13] c12;
wire          s13_16;
wire  [56:18] s13;
wire  [55:19] c13;
wire          s14_24;
wire  [64:26] s14;
wire  [63:27] c14;
assign
   q00_33 = ~p00[33],
   q01_33 = ~p01[33],
   q02_33 = ~p02[33],
   q03_33 = ~p03[33],
   q04_33 = ~p04[33],
   q05_33 = ~p05[33],
   q06_33 = ~p06[33],
   q07_33 = ~p07[33],
   q08_33 = ~p08[33],
   q09_33 = ~p09[33],
   q10_33 = ~p10[33],
   q11_33 = ~p11[33],
   q12_33 = ~p12[33],
   q13_33 = ~p13[33],
   q14_33 = ~p14[33],
   q15_33 = ~p15[33],
   q16_33 = ~p16[33];
buf     U1000c(    c10_00  ,                            neg00);
buf     U1000s(                s10[00],     p00[00]);
buf     U1001s(                s10[01],     p00[01]);
fp_add3 U1002 (.co(c10[03]),.s(s10[02]), .a(p00[02]),.b(p01[00]),.c(neg01));
fp_add3 U1003 (.co(c10[04]),.s(s10[03]), .a(p00[03]),.b(p01[01]),.c(  1'b0 ));
fp_add3 U1004 (.co(c10[05]),.s(s10[04]), .a(p00[04]),.b(p01[02]),.c(p02[00]));
fp_add3 U1005 (.co(c10[06]),.s(s10[05]), .a(p00[05]),.b(p01[03]),.c(p02[01]));
fp_add3 U1006 (.co(c10[07]),.s(s10[06]), .a(p00[06]),.b(p01[04]),.c(p02[02]));
fp_add3 U1007 (.co(c10[08]),.s(s10[07]), .a(p00[07]),.b(p01[05]),.c(p02[03]));
fp_add3 U1008 (.co(c10[09]),.s(s10[08]), .a(p00[08]),.b(p01[06]),.c(p02[04]));
fp_add3 U1009 (.co(c10[10]),.s(s10[09]), .a(p00[09]),.b(p01[07]),.c(p02[05]));
fp_add3 U1010 (.co(c10[11]),.s(s10[10]), .a(p00[10]),.b(p01[08]),.c(p02[06]));
fp_add3 U1011 (.co(c10[12]),.s(s10[11]), .a(p00[11]),.b(p01[09]),.c(p02[07]));
fp_add3 U1012 (.co(c10[13]),.s(s10[12]), .a(p00[12]),.b(p01[10]),.c(p02[08]));
fp_add3 U1013 (.co(c10[14]),.s(s10[13]), .a(p00[13]),.b(p01[11]),.c(p02[09]));
fp_add3 U1014 (.co(c10[15]),.s(s10[14]), .a(p00[14]),.b(p01[12]),.c(p02[10]));
fp_add3 U1015 (.co(c10[16]),.s(s10[15]), .a(p00[15]),.b(p01[13]),.c(p02[11]));
fp_add3 U1016 (.co(c10[17]),.s(s10[16]), .a(p00[16]),.b(p01[14]),.c(p02[12]));
fp_add3 U1017 (.co(c10[18]),.s(s10[17]), .a(p00[17]),.b(p01[15]),.c(p02[13]));
fp_add3 U1018 (.co(c10[19]),.s(s10[18]), .a(p00[18]),.b(p01[16]),.c(p02[14]));
fp_add3 U1019 (.co(c10[20]),.s(s10[19]), .a(p00[19]),.b(p01[17]),.c(p02[15]));
fp_add3 U1020 (.co(c10[21]),.s(s10[20]), .a(p00[20]),.b(p01[18]),.c(p02[16]));
fp_add3 U1021 (.co(c10[22]),.s(s10[21]), .a(p00[21]),.b(p01[19]),.c(p02[17]));
fp_add3 U1022 (.co(c10[23]),.s(s10[22]), .a(p00[22]),.b(p01[20]),.c(p02[18]));
fp_add3 U1023 (.co(c10[24]),.s(s10[23]), .a(p00[23]),.b(p01[21]),.c(p02[19]));
fp_add3 U1024 (.co(c10[25]),.s(s10[24]), .a(p00[24]),.b(p01[22]),.c(p02[20]));
fp_add3 U1025 (.co(c10[26]),.s(s10[25]), .a(p00[25]),.b(p01[23]),.c(p02[21]));
fp_add3 U1026 (.co(c10[27]),.s(s10[26]), .a(p00[26]),.b(p01[24]),.c(p02[22]));
fp_add3 U1027 (.co(c10[28]),.s(s10[27]), .a(p00[27]),.b(p01[25]),.c(p02[23]));
fp_add3 U1028 (.co(c10[29]),.s(s10[28]), .a(p00[28]),.b(p01[26]),.c(p02[24]));
fp_add3 U1029 (.co(c10[30]),.s(s10[29]), .a(p00[29]),.b(p01[27]),.c(p02[25]));
fp_add3 U1030 (.co(c10[31]),.s(s10[30]), .a(p00[30]),.b(p01[28]),.c(p02[26]));
fp_add3 U1031 (.co(c10[32]),.s(s10[31]), .a(p00[31]),.b(p01[29]),.c(p02[27]));
fp_add3 U1032 (.co(c10[33]),.s(s10[32]), .a(p00[32]),.b(p01[30]),.c(p02[28]));
fp_add3 U1033 (.co(c10[34]),.s(s10[33]), .a(p00[33]),.b(p01[31]),.c(p02[29]));
fp_add3 U1034 (.co(c10[35]),.s(s10[34]), .a(p00[33]),.b(p01[32]),.c(p02[30]));
fp_add3 U1035 (.co(c10[36]),.s(s10[35]), .a(q00_33 ),.b(q01_33 ),.c(p02[31]));
fp_add3 U1036 (.co(c10[37]),.s(s10[36]), .a(  1'b0 ),.b(  1'b1 ),.c(p02[32]));
buf     U1037 (                s10[37],                             q02_33  );
buf     U1038 (                s10[38],                               1'b1  );
buf     U1104 (                s11_04  ,     neg02 );
fp_add3 U1106 (.co(c11[07]),.s(s11[06]), .a(p03[00]),.b( neg03 ),.c(  1'b0 ));
fp_add3 U1107 (.co(c11[08]),.s(s11[07]), .a(p03[01]),.b(  1'b0 ),.c(  1'b0 ));
fp_add3 U1108 (.co(c11[09]),.s(s11[08]), .a(p03[02]),.b(p04[00]),.c( neg04 ));
fp_add3 U1109 (.co(c11[10]),.s(s11[09]), .a(p03[03]),.b(p04[01]),.c(  1'b0 ));
fp_add3 U1110 (.co(c11[11]),.s(s11[10]), .a(p03[04]),.b(p04[02]),.c(p05[00]));
fp_add3 U1111 (.co(c11[12]),.s(s11[11]), .a(p03[05]),.b(p04[03]),.c(p05[01]));
fp_add3 U1112 (.co(c11[13]),.s(s11[12]), .a(p03[06]),.b(p04[04]),.c(p05[02]));
fp_add3 U1113 (.co(c11[14]),.s(s11[13]), .a(p03[07]),.b(p04[05]),.c(p05[03]));
fp_add3 U1114 (.co(c11[15]),.s(s11[14]), .a(p03[08]),.b(p04[06]),.c(p05[04]));
fp_add3 U1115 (.co(c11[16]),.s(s11[15]), .a(p03[09]),.b(p04[07]),.c(p05[05]));
fp_add3 U1116 (.co(c11[17]),.s(s11[16]), .a(p03[10]),.b(p04[08]),.c(p05[06]));
fp_add3 U1117 (.co(c11[18]),.s(s11[17]), .a(p03[11]),.b(p04[09]),.c(p05[07]));
fp_add3 U1118 (.co(c11[19]),.s(s11[18]), .a(p03[12]),.b(p04[10]),.c(p05[08]));
fp_add3 U1119 (.co(c11[20]),.s(s11[19]), .a(p03[13]),.b(p04[11]),.c(p05[09]));
fp_add3 U1120 (.co(c11[21]),.s(s11[20]), .a(p03[14]),.b(p04[12]),.c(p05[10]));
fp_add3 U1121 (.co(c11[22]),.s(s11[21]), .a(p03[15]),.b(p04[13]),.c(p05[11]));
fp_add3 U1122 (.co(c11[23]),.s(s11[22]), .a(p03[16]),.b(p04[14]),.c(p05[12]));
fp_add3 U1123 (.co(c11[24]),.s(s11[23]), .a(p03[17]),.b(p04[15]),.c(p05[13]));
fp_add3 U1124 (.co(c11[25]),.s(s11[24]), .a(p03[18]),.b(p04[16]),.c(p05[14]));
fp_add3 U1125 (.co(c11[26]),.s(s11[25]), .a(p03[19]),.b(p04[17]),.c(p05[15]));
fp_add3 U1126 (.co(c11[27]),.s(s11[26]), .a(p03[20]),.b(p04[18]),.c(p05[16]));
fp_add3 U1127 (.co(c11[28]),.s(s11[27]), .a(p03[21]),.b(p04[19]),.c(p05[17]));
fp_add3 U1128 (.co(c11[29]),.s(s11[28]), .a(p03[22]),.b(p04[20]),.c(p05[18]));
fp_add3 U1129 (.co(c11[30]),.s(s11[29]), .a(p03[23]),.b(p04[21]),.c(p05[19]));
fp_add3 U1130 (.co(c11[31]),.s(s11[30]), .a(p03[24]),.b(p04[22]),.c(p05[20]));
fp_add3 U1131 (.co(c11[32]),.s(s11[31]), .a(p03[25]),.b(p04[23]),.c(p05[21]));
fp_add3 U1132 (.co(c11[33]),.s(s11[32]), .a(p03[26]),.b(p04[24]),.c(p05[22]));
fp_add3 U1133 (.co(c11[34]),.s(s11[33]), .a(p03[27]),.b(p04[25]),.c(p05[23]));
fp_add3 U1134 (.co(c11[35]),.s(s11[34]), .a(p03[28]),.b(p04[26]),.c(p05[24]));
fp_add3 U1135 (.co(c11[36]),.s(s11[35]), .a(p03[29]),.b(p04[27]),.c(p05[25]));
fp_add3 U1136 (.co(c11[37]),.s(s11[36]), .a(p03[30]),.b(p04[28]),.c(p05[26]));
fp_add3 U1137 (.co(c11[38]),.s(s11[37]), .a(p03[31]),.b(p04[29]),.c(p05[27]));
fp_add3 U1138 (.co(c11[39]),.s(s11[38]), .a(p03[32]),.b(p04[30]),.c(p05[28]));
fp_add3 U1139 (.co(c11[40]),.s(s11[39]), .a(q03_33 ),.b(p04[31]),.c(p05[29]));
fp_add3 U1140 (.co(c11[41]),.s(s11[40]), .a(  1'b1 ),.b(p04[32]),.c(p05[30]));
fp_add3 U1141 (.co(c11[42]),.s(s11[41]), .a(  1'b0 ),.b(q04_33 ),.c(p05[31]));
fp_add3 U1142 (.co(c11[43]),.s(s11[42]), .a(  1'b0 ),.b(  1'b1 ),.c(p05[32]));
buf     U1143 (                s11[43],                             q05_33  );
buf     U1144 (                s11[44],                               1'b1  );
buf     U1210 (                s12_10  ,     neg05 );
fp_add3 U1212 (.co(c12[13]),.s(s12[12]), .a(p06[00]),.b( neg06 ),.c(  1'b0 ));
fp_add3 U1213 (.co(c12[14]),.s(s12[13]), .a(p06[01]),.b(  1'b0 ),.c(  1'b0 ));
fp_add3 U1214 (.co(c12[15]),.s(s12[14]), .a(p06[02]),.b(p07[00]),.c( neg07 ));
fp_add3 U1215 (.co(c12[16]),.s(s12[15]), .a(p06[03]),.b(p07[01]),.c(  1'b0 ));
fp_add3 U1216 (.co(c12[17]),.s(s12[16]), .a(p06[04]),.b(p07[02]),.c(p08[00]));
fp_add3 U1217 (.co(c12[18]),.s(s12[17]), .a(p06[05]),.b(p07[03]),.c(p08[01]));
fp_add3 U1218 (.co(c12[19]),.s(s12[18]), .a(p06[06]),.b(p07[04]),.c(p08[02]));
fp_add3 U1219 (.co(c12[20]),.s(s12[19]), .a(p06[07]),.b(p07[05]),.c(p08[03]));
fp_add3 U1220 (.co(c12[21]),.s(s12[20]), .a(p06[08]),.b(p07[06]),.c(p08[04]));
fp_add3 U1221 (.co(c12[22]),.s(s12[21]), .a(p06[09]),.b(p07[07]),.c(p08[05]));
fp_add3 U1222 (.co(c12[23]),.s(s12[22]), .a(p06[10]),.b(p07[08]),.c(p08[06]));
fp_add3 U1223 (.co(c12[24]),.s(s12[23]), .a(p06[11]),.b(p07[09]),.c(p08[07]));
fp_add3 U1224 (.co(c12[25]),.s(s12[24]), .a(p06[12]),.b(p07[10]),.c(p08[08]));
fp_add3 U1225 (.co(c12[26]),.s(s12[25]), .a(p06[13]),.b(p07[11]),.c(p08[09]));
fp_add3 U1226 (.co(c12[27]),.s(s12[26]), .a(p06[14]),.b(p07[12]),.c(p08[10]));
fp_add3 U1227 (.co(c12[28]),.s(s12[27]), .a(p06[15]),.b(p07[13]),.c(p08[11]));
fp_add3 U1228 (.co(c12[29]),.s(s12[28]), .a(p06[16]),.b(p07[14]),.c(p08[12]));
fp_add3 U1229 (.co(c12[30]),.s(s12[29]), .a(p06[17]),.b(p07[15]),.c(p08[13]));
fp_add3 U1230 (.co(c12[31]),.s(s12[30]), .a(p06[18]),.b(p07[16]),.c(p08[14]));
fp_add3 U1231 (.co(c12[32]),.s(s12[31]), .a(p06[19]),.b(p07[17]),.c(p08[15]));
fp_add3 U1232 (.co(c12[33]),.s(s12[32]), .a(p06[20]),.b(p07[18]),.c(p08[16]));
fp_add3 U1233 (.co(c12[34]),.s(s12[33]), .a(p06[21]),.b(p07[19]),.c(p08[17]));
fp_add3 U1234 (.co(c12[35]),.s(s12[34]), .a(p06[22]),.b(p07[20]),.c(p08[18]));
fp_add3 U1235 (.co(c12[36]),.s(s12[35]), .a(p06[23]),.b(p07[21]),.c(p08[19]));
fp_add3 U1236 (.co(c12[37]),.s(s12[36]), .a(p06[24]),.b(p07[22]),.c(p08[20]));
fp_add3 U1237 (.co(c12[38]),.s(s12[37]), .a(p06[25]),.b(p07[23]),.c(p08[21]));
fp_add3 U1238 (.co(c12[39]),.s(s12[38]), .a(p06[26]),.b(p07[24]),.c(p08[22]));
fp_add3 U1239 (.co(c12[40]),.s(s12[39]), .a(p06[27]),.b(p07[25]),.c(p08[23]));
fp_add3 U1240 (.co(c12[41]),.s(s12[40]), .a(p06[28]),.b(p07[26]),.c(p08[24]));
fp_add3 U1241 (.co(c12[42]),.s(s12[41]), .a(p06[29]),.b(p07[27]),.c(p08[25]));
fp_add3 U1242 (.co(c12[43]),.s(s12[42]), .a(p06[30]),.b(p07[28]),.c(p08[26]));
fp_add3 U1243 (.co(c12[44]),.s(s12[43]), .a(p06[31]),.b(p07[29]),.c(p08[27]));
fp_add3 U1244 (.co(c12[45]),.s(s12[44]), .a(p06[32]),.b(p07[30]),.c(p08[28]));
fp_add3 U1245 (.co(c12[46]),.s(s12[45]), .a(q06_33 ),.b(p07[31]),.c(p08[29]));
fp_add3 U1246 (.co(c12[47]),.s(s12[46]), .a(  1'b1 ),.b(p07[32]),.c(p08[30]));
fp_add3 U1247 (.co(c12[48]),.s(s12[47]), .a(  1'b0 ),.b(q07_33 ),.c(p08[31]));
fp_add3 U1248 (.co(c12[49]),.s(s12[48]), .a(  1'b0 ),.b(  1'b1 ),.c(p08[32]));
buf     U1249 (                s12[49],                             q08_33  );
buf     U1250 (                s12[50],                               1'b1  );
buf     U1316 (                s13_16  ,     neg08 );
fp_add3 U1318 (.co(c13[19]),.s(s13[18]), .a(p09[00]),.b( neg09 ),.c(  1'b0 ));
fp_add3 U1319 (.co(c13[20]),.s(s13[19]), .a(p09[01]),.b(  1'b0 ),.c(  1'b0 ));
fp_add3 U1320 (.co(c13[21]),.s(s13[20]), .a(p09[02]),.b(p10[00]),.c( neg10 ));
fp_add3 U1321 (.co(c13[22]),.s(s13[21]), .a(p09[03]),.b(p10[01]),.c(  1'b0 ));
fp_add3 U1322 (.co(c13[23]),.s(s13[22]), .a(p09[04]),.b(p10[02]),.c(p11[00]));
fp_add3 U1323 (.co(c13[24]),.s(s13[23]), .a(p09[05]),.b(p10[03]),.c(p11[01]));
fp_add3 U1324 (.co(c13[25]),.s(s13[24]), .a(p09[06]),.b(p10[04]),.c(p11[02]));
fp_add3 U1325 (.co(c13[26]),.s(s13[25]), .a(p09[07]),.b(p10[05]),.c(p11[03]));
fp_add3 U1326 (.co(c13[27]),.s(s13[26]), .a(p09[08]),.b(p10[06]),.c(p11[04]));
fp_add3 U1327 (.co(c13[28]),.s(s13[27]), .a(p09[09]),.b(p10[07]),.c(p11[05]));
fp_add3 U1328 (.co(c13[29]),.s(s13[28]), .a(p09[10]),.b(p10[08]),.c(p11[06]));
fp_add3 U1329 (.co(c13[30]),.s(s13[29]), .a(p09[11]),.b(p10[09]),.c(p11[07]));
fp_add3 U1330 (.co(c13[31]),.s(s13[30]), .a(p09[12]),.b(p10[10]),.c(p11[08]));
fp_add3 U1331 (.co(c13[32]),.s(s13[31]), .a(p09[13]),.b(p10[11]),.c(p11[09]));
fp_add3 U1332 (.co(c13[33]),.s(s13[32]), .a(p09[14]),.b(p10[12]),.c(p11[10]));
fp_add3 U1333 (.co(c13[34]),.s(s13[33]), .a(p09[15]),.b(p10[13]),.c(p11[11]));
fp_add3 U1334 (.co(c13[35]),.s(s13[34]), .a(p09[16]),.b(p10[14]),.c(p11[12]));
fp_add3 U1335 (.co(c13[36]),.s(s13[35]), .a(p09[17]),.b(p10[15]),.c(p11[13]));
fp_add3 U1336 (.co(c13[37]),.s(s13[36]), .a(p09[18]),.b(p10[16]),.c(p11[14]));
fp_add3 U1337 (.co(c13[38]),.s(s13[37]), .a(p09[19]),.b(p10[17]),.c(p11[15]));
fp_add3 U1338 (.co(c13[39]),.s(s13[38]), .a(p09[20]),.b(p10[18]),.c(p11[16]));
fp_add3 U1339 (.co(c13[40]),.s(s13[39]), .a(p09[21]),.b(p10[19]),.c(p11[17]));
fp_add3 U1340 (.co(c13[41]),.s(s13[40]), .a(p09[22]),.b(p10[20]),.c(p11[18]));
fp_add3 U1341 (.co(c13[42]),.s(s13[41]), .a(p09[23]),.b(p10[21]),.c(p11[19]));
fp_add3 U1342 (.co(c13[43]),.s(s13[42]), .a(p09[24]),.b(p10[22]),.c(p11[20]));
fp_add3 U1343 (.co(c13[44]),.s(s13[43]), .a(p09[25]),.b(p10[23]),.c(p11[21]));
fp_add3 U1344 (.co(c13[45]),.s(s13[44]), .a(p09[26]),.b(p10[24]),.c(p11[22]));
fp_add3 U1345 (.co(c13[46]),.s(s13[45]), .a(p09[27]),.b(p10[25]),.c(p11[23]));
fp_add3 U1346 (.co(c13[47]),.s(s13[46]), .a(p09[28]),.b(p10[26]),.c(p11[24]));
fp_add3 U1347 (.co(c13[48]),.s(s13[47]), .a(p09[29]),.b(p10[27]),.c(p11[25]));
fp_add3 U1348 (.co(c13[49]),.s(s13[48]), .a(p09[30]),.b(p10[28]),.c(p11[26]));
fp_add3 U1349 (.co(c13[50]),.s(s13[49]), .a(p09[31]),.b(p10[29]),.c(p11[27]));
fp_add3 U1350 (.co(c13[51]),.s(s13[50]), .a(p09[32]),.b(p10[30]),.c(p11[28]));
fp_add3 U1351 (.co(c13[52]),.s(s13[51]), .a(q09_33 ),.b(p10[31]),.c(p11[29]));
fp_add3 U1352 (.co(c13[53]),.s(s13[52]), .a(  1'b1 ),.b(p10[32]),.c(p11[30]));
fp_add3 U1353 (.co(c13[54]),.s(s13[53]), .a(  1'b0 ),.b(q10_33 ),.c(p11[31]));
fp_add3 U1354 (.co(c13[55]),.s(s13[54]), .a(  1'b0 ),.b(  1'b1 ),.c(p11[32]));
buf     U1355 (                s13[55],                             q11_33  );
buf     U1356 (                s13[56],                               1'b1  );
buf     U1424 (                s14_24  ,     neg12 );
fp_add3 U1426 (.co(c14[27]),.s(s14[26]), .a(p13[00]),.b( neg13 ),.c(  1'b0 ));
fp_add3 U1427 (.co(c14[28]),.s(s14[27]), .a(p13[01]),.b(  1'b0 ),.c(  1'b0 ));
fp_add3 U1428 (.co(c14[29]),.s(s14[28]), .a(p13[02]),.b(p14[00]),.c( neg14 ));
fp_add3 U1429 (.co(c14[30]),.s(s14[29]), .a(p13[03]),.b(p14[01]),.c(  1'b0 ));
fp_add3 U1430 (.co(c14[31]),.s(s14[30]), .a(p13[04]),.b(p14[02]),.c(p15[00]));
fp_add3 U1431 (.co(c14[32]),.s(s14[31]), .a(p13[05]),.b(p14[03]),.c(p15[01]));
fp_add3 U1432 (.co(c14[33]),.s(s14[32]), .a(p13[06]),.b(p14[04]),.c(p15[02]));
fp_add3 U1433 (.co(c14[34]),.s(s14[33]), .a(p13[07]),.b(p14[05]),.c(p15[03]));
fp_add3 U1434 (.co(c14[35]),.s(s14[34]), .a(p13[08]),.b(p14[06]),.c(p15[04]));
fp_add3 U1435 (.co(c14[36]),.s(s14[35]), .a(p13[09]),.b(p14[07]),.c(p15[05]));
fp_add3 U1436 (.co(c14[37]),.s(s14[36]), .a(p13[10]),.b(p14[08]),.c(p15[06]));
fp_add3 U1437 (.co(c14[38]),.s(s14[37]), .a(p13[11]),.b(p14[09]),.c(p15[07]));
fp_add3 U1438 (.co(c14[39]),.s(s14[38]), .a(p13[12]),.b(p14[10]),.c(p15[08]));
fp_add3 U1439 (.co(c14[40]),.s(s14[39]), .a(p13[13]),.b(p14[11]),.c(p15[09]));
fp_add3 U1440 (.co(c14[41]),.s(s14[40]), .a(p13[14]),.b(p14[12]),.c(p15[10]));
fp_add3 U1441 (.co(c14[42]),.s(s14[41]), .a(p13[15]),.b(p14[13]),.c(p15[11]));
fp_add3 U1442 (.co(c14[43]),.s(s14[42]), .a(p13[16]),.b(p14[14]),.c(p15[12]));
fp_add3 U1443 (.co(c14[44]),.s(s14[43]), .a(p13[17]),.b(p14[15]),.c(p15[13]));
fp_add3 U1444 (.co(c14[45]),.s(s14[44]), .a(p13[18]),.b(p14[16]),.c(p15[14]));
fp_add3 U1445 (.co(c14[46]),.s(s14[45]), .a(p13[19]),.b(p14[17]),.c(p15[15]));
fp_add3 U1446 (.co(c14[47]),.s(s14[46]), .a(p13[20]),.b(p14[18]),.c(p15[16]));
fp_add3 U1447 (.co(c14[48]),.s(s14[47]), .a(p13[21]),.b(p14[19]),.c(p15[17]));
fp_add3 U1448 (.co(c14[49]),.s(s14[48]), .a(p13[22]),.b(p14[20]),.c(p15[18]));
fp_add3 U1449 (.co(c14[50]),.s(s14[49]), .a(p13[23]),.b(p14[21]),.c(p15[19]));
fp_add3 U1450 (.co(c14[51]),.s(s14[50]), .a(p13[24]),.b(p14[22]),.c(p15[20]));
fp_add3 U1451 (.co(c14[52]),.s(s14[51]), .a(p13[25]),.b(p14[23]),.c(p15[21]));
fp_add3 U1452 (.co(c14[53]),.s(s14[52]), .a(p13[26]),.b(p14[24]),.c(p15[22]));
fp_add3 U1453 (.co(c14[54]),.s(s14[53]), .a(p13[27]),.b(p14[25]),.c(p15[23]));
fp_add3 U1454 (.co(c14[55]),.s(s14[54]), .a(p13[28]),.b(p14[26]),.c(p15[24]));
fp_add3 U1455 (.co(c14[56]),.s(s14[55]), .a(p13[29]),.b(p14[27]),.c(p15[25]));
fp_add3 U1456 (.co(c14[57]),.s(s14[56]), .a(p13[30]),.b(p14[28]),.c(p15[26]));
fp_add3 U1457 (.co(c14[58]),.s(s14[57]), .a(p13[31]),.b(p14[29]),.c(p15[27]));
fp_add3 U1458 (.co(c14[59]),.s(s14[58]), .a(p13[32]),.b(p14[30]),.c(p15[28]));
fp_add3 U1459 (.co(c14[60]),.s(s14[59]), .a(q13_33 ),.b(p14[31]),.c(p15[29]));
fp_add3 U1460 (.co(c14[61]),.s(s14[60]), .a(  1'b1 ),.b(p14[32]),.c(p15[30]));
fp_add3 U1461 (.co(c14[62]),.s(s14[61]), .a(  1'b0 ),.b(q14_33 ),.c(p15[31]));
fp_add3 U1462 (.co(c14[63]),.s(s14[62]), .a(  1'b0 ),.b(  1'b1 ),.c(p15[32]));
buf     U1463 (                s14[63],                             q15_33  );
buf     U1464 (                s14[64],                               1'b1  );
buf     U2000s(                s20[00] ,    s10[00]);
buf     U2000c(    c20_00  ,                            c10_00 );
buf     U2001 (                s20[01] ,    s10[01]);
buf     U2002 (                s20[02] ,    s10[02]);
buf     U2003s(                s20[03] ,    s10[03]);
buf     U2003c(    c20_03  ,                            c10[03]);
fp_add3 U2004 (.co(c20[05]),.s(s20[04]), .a(s10[04]),.b(c10[04]),.c(s11_04 ));
fp_add3 U2005 (.co(c20[06]),.s(s20[05]), .a(s10[05]),.b(c10[05]),.c(  1'b0 ));
fp_add3 U2006 (.co(c20[07]),.s(s20[06]), .a(s10[06]),.b(c10[06]),.c(s11[06]));
fp_add3 U2007 (.co(c20[08]),.s(s20[07]), .a(s10[07]),.b(c10[07]),.c(s11[07]));
fp_add3 U2008 (.co(c20[09]),.s(s20[08]), .a(s10[08]),.b(c10[08]),.c(s11[08]));
fp_add3 U2009 (.co(c20[10]),.s(s20[09]), .a(s10[09]),.b(c10[09]),.c(s11[09]));
fp_add3 U2010 (.co(c20[11]),.s(s20[10]), .a(s10[10]),.b(c10[10]),.c(s11[10]));
fp_add3 U2011 (.co(c20[12]),.s(s20[11]), .a(s10[11]),.b(c10[11]),.c(s11[11]));
fp_add3 U2012 (.co(c20[13]),.s(s20[12]), .a(s10[12]),.b(c10[12]),.c(s11[12]));
fp_add3 U2013 (.co(c20[14]),.s(s20[13]), .a(s10[13]),.b(c10[13]),.c(s11[13]));
fp_add3 U2014 (.co(c20[15]),.s(s20[14]), .a(s10[14]),.b(c10[14]),.c(s11[14]));
fp_add3 U2015 (.co(c20[16]),.s(s20[15]), .a(s10[15]),.b(c10[15]),.c(s11[15]));
fp_add3 U2016 (.co(c20[17]),.s(s20[16]), .a(s10[16]),.b(c10[16]),.c(s11[16]));
fp_add3 U2017 (.co(c20[18]),.s(s20[17]), .a(s10[17]),.b(c10[17]),.c(s11[17]));
fp_add3 U2018 (.co(c20[19]),.s(s20[18]), .a(s10[18]),.b(c10[18]),.c(s11[18]));
fp_add3 U2019 (.co(c20[20]),.s(s20[19]), .a(s10[19]),.b(c10[19]),.c(s11[19]));
fp_add3 U2020 (.co(c20[21]),.s(s20[20]), .a(s10[20]),.b(c10[20]),.c(s11[20]));
fp_add3 U2021 (.co(c20[22]),.s(s20[21]), .a(s10[21]),.b(c10[21]),.c(s11[21]));
fp_add3 U2022 (.co(c20[23]),.s(s20[22]), .a(s10[22]),.b(c10[22]),.c(s11[22]));
fp_add3 U2023 (.co(c20[24]),.s(s20[23]), .a(s10[23]),.b(c10[23]),.c(s11[23]));
fp_add3 U2024 (.co(c20[25]),.s(s20[24]), .a(s10[24]),.b(c10[24]),.c(s11[24]));
fp_add3 U2025 (.co(c20[26]),.s(s20[25]), .a(s10[25]),.b(c10[25]),.c(s11[25]));
fp_add3 U2026 (.co(c20[27]),.s(s20[26]), .a(s10[26]),.b(c10[26]),.c(s11[26]));
fp_add3 U2027 (.co(c20[28]),.s(s20[27]), .a(s10[27]),.b(c10[27]),.c(s11[27]));
fp_add3 U2028 (.co(c20[29]),.s(s20[28]), .a(s10[28]),.b(c10[28]),.c(s11[28]));
fp_add3 U2029 (.co(c20[30]),.s(s20[29]), .a(s10[29]),.b(c10[29]),.c(s11[29]));
fp_add3 U2030 (.co(c20[31]),.s(s20[30]), .a(s10[30]),.b(c10[30]),.c(s11[30]));
fp_add3 U2031 (.co(c20[32]),.s(s20[31]), .a(s10[31]),.b(c10[31]),.c(s11[31]));
fp_add3 U2032 (.co(c20[33]),.s(s20[32]), .a(s10[32]),.b(c10[32]),.c(s11[32]));
fp_add3 U2033 (.co(c20[34]),.s(s20[33]), .a(s10[33]),.b(c10[33]),.c(s11[33]));
fp_add3 U2034 (.co(c20[35]),.s(s20[34]), .a(s10[34]),.b(c10[34]),.c(s11[34]));
fp_add3 U2035 (.co(c20[36]),.s(s20[35]), .a(s10[35]),.b(c10[35]),.c(s11[35]));
fp_add3 U2036 (.co(c20[37]),.s(s20[36]), .a(s10[36]),.b(c10[36]),.c(s11[36]));
fp_add3 U2037 (.co(c20[38]),.s(s20[37]), .a(s10[37]),.b(c10[37]),.c(s11[37]));
fp_add3 U2038 (.co(c20[39]),.s(s20[38]), .a(s10[38]),.b(  1'b0 ),.c(s11[38]));
buf     U2039 (                s20[39] ,                            s11[39]);
buf     U2040 (                s20[40] ,                            s11[40]);
buf     U2041 (                s20[41] ,                            s11[41]);
buf     U2042 (                s20[42] ,                            s11[42]);
buf     U2043 (                s20[43] ,                            s11[43]);
buf     U2044 (                s20[44] ,                            s11[44]);
buf     U2107 (                s21[07] ,    c11[07]);
buf     U2108 (                s21[08] ,    c11[08]);
buf     U2109 (                s21[09] ,    c11[09]);
fp_add3 U2110 (.co(c21[11]),.s(s21[10]), .a(c11[10]),.b(s12_10 ),.c(  1'b0 ));
fp_add3 U2111 (.co(c21[12]),.s(s21[11]), .a(c11[11]),.b(  1'b0 ),.c(  1'b0 ));
fp_add3 U2112 (.co(c21[13]),.s(s21[12]), .a(c11[12]),.b(s12[12]),.c(  1'b0 ));
fp_add3 U2113 (.co(c21[14]),.s(s21[13]), .a(c11[13]),.b(s12[13]),.c(c12[13]));
fp_add3 U2114 (.co(c21[15]),.s(s21[14]), .a(c11[14]),.b(s12[14]),.c(c12[14]));
fp_add3 U2115 (.co(c21[16]),.s(s21[15]), .a(c11[15]),.b(s12[15]),.c(c12[15]));
fp_add3 U2116 (.co(c21[17]),.s(s21[16]), .a(c11[16]),.b(s12[16]),.c(c12[16]));
fp_add3 U2117 (.co(c21[18]),.s(s21[17]), .a(c11[17]),.b(s12[17]),.c(c12[17]));
fp_add3 U2118 (.co(c21[19]),.s(s21[18]), .a(c11[18]),.b(s12[18]),.c(c12[18]));
fp_add3 U2119 (.co(c21[20]),.s(s21[19]), .a(c11[19]),.b(s12[19]),.c(c12[19]));
fp_add3 U2120 (.co(c21[21]),.s(s21[20]), .a(c11[20]),.b(s12[20]),.c(c12[20]));
fp_add3 U2121 (.co(c21[22]),.s(s21[21]), .a(c11[21]),.b(s12[21]),.c(c12[21]));
fp_add3 U2122 (.co(c21[23]),.s(s21[22]), .a(c11[22]),.b(s12[22]),.c(c12[22]));
fp_add3 U2123 (.co(c21[24]),.s(s21[23]), .a(c11[23]),.b(s12[23]),.c(c12[23]));
fp_add3 U2124 (.co(c21[25]),.s(s21[24]), .a(c11[24]),.b(s12[24]),.c(c12[24]));
fp_add3 U2125 (.co(c21[26]),.s(s21[25]), .a(c11[25]),.b(s12[25]),.c(c12[25]));
fp_add3 U2126 (.co(c21[27]),.s(s21[26]), .a(c11[26]),.b(s12[26]),.c(c12[26]));
fp_add3 U2127 (.co(c21[28]),.s(s21[27]), .a(c11[27]),.b(s12[27]),.c(c12[27]));
fp_add3 U2128 (.co(c21[29]),.s(s21[28]), .a(c11[28]),.b(s12[28]),.c(c12[28]));
fp_add3 U2129 (.co(c21[30]),.s(s21[29]), .a(c11[29]),.b(s12[29]),.c(c12[29]));
fp_add3 U2130 (.co(c21[31]),.s(s21[30]), .a(c11[30]),.b(s12[30]),.c(c12[30]));
fp_add3 U2131 (.co(c21[32]),.s(s21[31]), .a(c11[31]),.b(s12[31]),.c(c12[31]));
fp_add3 U2132 (.co(c21[33]),.s(s21[32]), .a(c11[32]),.b(s12[32]),.c(c12[32]));
fp_add3 U2133 (.co(c21[34]),.s(s21[33]), .a(c11[33]),.b(s12[33]),.c(c12[33]));
fp_add3 U2134 (.co(c21[35]),.s(s21[34]), .a(c11[34]),.b(s12[34]),.c(c12[34]));
fp_add3 U2135 (.co(c21[36]),.s(s21[35]), .a(c11[35]),.b(s12[35]),.c(c12[35]));
fp_add3 U2136 (.co(c21[37]),.s(s21[36]), .a(c11[36]),.b(s12[36]),.c(c12[36]));
fp_add3 U2137 (.co(c21[38]),.s(s21[37]), .a(c11[37]),.b(s12[37]),.c(c12[37]));
fp_add3 U2138 (.co(c21[39]),.s(s21[38]), .a(c11[38]),.b(s12[38]),.c(c12[38]));
fp_add3 U2139 (.co(c21[40]),.s(s21[39]), .a(c11[39]),.b(s12[39]),.c(c12[39]));
fp_add3 U2140 (.co(c21[41]),.s(s21[40]), .a(c11[40]),.b(s12[40]),.c(c12[40]));
fp_add3 U2141 (.co(c21[42]),.s(s21[41]), .a(c11[41]),.b(s12[41]),.c(c12[41]));
fp_add3 U2142 (.co(c21[43]),.s(s21[42]), .a(c11[42]),.b(s12[42]),.c(c12[42]));
fp_add3 U2143 (.co(c21[44]),.s(s21[43]), .a(c11[43]),.b(s12[43]),.c(c12[43]));
fp_add3 U2144 (.co(c21[45]),.s(s21[44]), .a(  1'b0 ),.b(s12[44]),.c(c12[44]));
fp_add3 U2145 (.co(c21[46]),.s(s21[45]), .a(  1'b0 ),.b(s12[45]),.c(c12[45]));
fp_add3 U2146 (.co(c21[47]),.s(s21[46]), .a(  1'b0 ),.b(s12[46]),.c(c12[46]));
fp_add3 U2147 (.co(c21[48]),.s(s21[47]), .a(  1'b0 ),.b(s12[47]),.c(c12[47]));
fp_add3 U2148 (.co(c21[49]),.s(s21[48]), .a(  1'b0 ),.b(s12[48]),.c(c12[48]));
fp_add3 U2149 (.co(c21[50]),.s(s21[49]), .a(  1'b0 ),.b(s12[49]),.c(c12[49]));
buf     U2150 (                s21[50] ,                s12[50]);
buf     U2216 (                s22_16  ,    s13_16 );
buf     U2218 (                s22[18] ,    s13[18]);
fp_add3 U2219 (.co(c22[20]),.s(s22[19]), .a(s13[19]),.b(c13[19]),.c(  1'b0 ));
fp_add3 U2220 (.co(c22[21]),.s(s22[20]), .a(s13[20]),.b(c13[20]),.c(  1'b0 ));
fp_add3 U2221 (.co(c22[22]),.s(s22[21]), .a(s13[21]),.b(c13[21]),.c(  1'b0 ));
fp_add3 U2222 (.co(c22[23]),.s(s22[22]), .a(s13[22]),.b(c13[22]),.c( neg11 ));
fp_add3 U2223 (.co(c22[24]),.s(s22[23]), .a(s13[23]),.b(c13[23]),.c(  1'b0 ));
fp_add3 U2224 (.co(c22[25]),.s(s22[24]), .a(s13[24]),.b(c13[24]),.c(p12[00]));
fp_add3 U2225 (.co(c22[26]),.s(s22[25]), .a(s13[25]),.b(c13[25]),.c(p12[01]));
fp_add3 U2226 (.co(c22[27]),.s(s22[26]), .a(s13[26]),.b(c13[26]),.c(p12[02]));
fp_add3 U2227 (.co(c22[28]),.s(s22[27]), .a(s13[27]),.b(c13[27]),.c(p12[03]));
fp_add3 U2228 (.co(c22[29]),.s(s22[28]), .a(s13[28]),.b(c13[28]),.c(p12[04]));
fp_add3 U2229 (.co(c22[30]),.s(s22[29]), .a(s13[29]),.b(c13[29]),.c(p12[05]));
fp_add3 U2230 (.co(c22[31]),.s(s22[30]), .a(s13[30]),.b(c13[30]),.c(p12[06]));
fp_add3 U2231 (.co(c22[32]),.s(s22[31]), .a(s13[31]),.b(c13[31]),.c(p12[07]));
fp_add3 U2232 (.co(c22[33]),.s(s22[32]), .a(s13[32]),.b(c13[32]),.c(p12[08]));
fp_add3 U2233 (.co(c22[34]),.s(s22[33]), .a(s13[33]),.b(c13[33]),.c(p12[09]));
fp_add3 U2234 (.co(c22[35]),.s(s22[34]), .a(s13[34]),.b(c13[34]),.c(p12[10]));
fp_add3 U2235 (.co(c22[36]),.s(s22[35]), .a(s13[35]),.b(c13[35]),.c(p12[11]));
fp_add3 U2236 (.co(c22[37]),.s(s22[36]), .a(s13[36]),.b(c13[36]),.c(p12[12]));
fp_add3 U2237 (.co(c22[38]),.s(s22[37]), .a(s13[37]),.b(c13[37]),.c(p12[13]));
fp_add3 U2238 (.co(c22[39]),.s(s22[38]), .a(s13[38]),.b(c13[38]),.c(p12[14]));
fp_add3 U2239 (.co(c22[40]),.s(s22[39]), .a(s13[39]),.b(c13[39]),.c(p12[15]));
fp_add3 U2240 (.co(c22[41]),.s(s22[40]), .a(s13[40]),.b(c13[40]),.c(p12[16]));
fp_add3 U2241 (.co(c22[42]),.s(s22[41]), .a(s13[41]),.b(c13[41]),.c(p12[17]));
fp_add3 U2242 (.co(c22[43]),.s(s22[42]), .a(s13[42]),.b(c13[42]),.c(p12[18]));
fp_add3 U2243 (.co(c22[44]),.s(s22[43]), .a(s13[43]),.b(c13[43]),.c(p12[19]));
fp_add3 U2244 (.co(c22[45]),.s(s22[44]), .a(s13[44]),.b(c13[44]),.c(p12[20]));
fp_add3 U2245 (.co(c22[46]),.s(s22[45]), .a(s13[45]),.b(c13[45]),.c(p12[21]));
fp_add3 U2246 (.co(c22[47]),.s(s22[46]), .a(s13[46]),.b(c13[46]),.c(p12[22]));
fp_add3 U2247 (.co(c22[48]),.s(s22[47]), .a(s13[47]),.b(c13[47]),.c(p12[23]));
fp_add3 U2248 (.co(c22[49]),.s(s22[48]), .a(s13[48]),.b(c13[48]),.c(p12[24]));
fp_add3 U2249 (.co(c22[50]),.s(s22[49]), .a(s13[49]),.b(c13[49]),.c(p12[25]));
fp_add3 U2250 (.co(c22[51]),.s(s22[50]), .a(s13[50]),.b(c13[50]),.c(p12[26]));
fp_add3 U2251 (.co(c22[52]),.s(s22[51]), .a(s13[51]),.b(c13[51]),.c(p12[27]));
fp_add3 U2252 (.co(c22[53]),.s(s22[52]), .a(s13[52]),.b(c13[52]),.c(p12[28]));
fp_add3 U2253 (.co(c22[54]),.s(s22[53]), .a(s13[53]),.b(c13[53]),.c(p12[29]));
fp_add3 U2254 (.co(c22[55]),.s(s22[54]), .a(s13[54]),.b(c13[54]),.c(p12[30]));
fp_add3 U2255 (.co(c22[56]),.s(s22[55]), .a(s13[55]),.b(c13[55]),.c(p12[31]));
fp_add3 U2256 (.co(c22[57]),.s(s22[56]), .a(s13[56]),.b(  1'b0 ),.c(p12[32]));
buf     U2257 (                s22[57] ,                            q12_33  );
buf     U2324 (                s23_24  ,    s14_24 );
buf     U2326 (                s23[26] ,    s14[26]);
fp_add3 U2327 (.co(c23[28]),.s(s23[27]), .a(s14[27]),.b(c14[27]),.c(  1'b0 ));
fp_add3 U2328 (.co(c23[29]),.s(s23[28]), .a(s14[28]),.b(c14[28]),.c(  1'b0 ));
fp_add3 U2329 (.co(c23[30]),.s(s23[29]), .a(s14[29]),.b(c14[29]),.c(  1'b0 ));
fp_add3 U2330 (.co(c23[31]),.s(s23[30]), .a(s14[30]),.b(c14[30]),.c( neg15 ));
fp_add3 U2331 (.co(c23[32]),.s(s23[31]), .a(s14[31]),.b(c14[31]),.c(  1'b0 ));
fp_add3 U2332 (.co(c23[33]),.s(s23[32]), .a(s14[32]),.b(c14[32]),.c(p16[00]));
fp_add3 U2333 (.co(c23[34]),.s(s23[33]), .a(s14[33]),.b(c14[33]),.c(p16[01]));
fp_add3 U2334 (.co(c23[35]),.s(s23[34]), .a(s14[34]),.b(c14[34]),.c(p16[02]));
fp_add3 U2335 (.co(c23[36]),.s(s23[35]), .a(s14[35]),.b(c14[35]),.c(p16[03]));
fp_add3 U2336 (.co(c23[37]),.s(s23[36]), .a(s14[36]),.b(c14[36]),.c(p16[04]));
fp_add3 U2337 (.co(c23[38]),.s(s23[37]), .a(s14[37]),.b(c14[37]),.c(p16[05]));
fp_add3 U2338 (.co(c23[39]),.s(s23[38]), .a(s14[38]),.b(c14[38]),.c(p16[06]));
fp_add3 U2339 (.co(c23[40]),.s(s23[39]), .a(s14[39]),.b(c14[39]),.c(p16[07]));
fp_add3 U2340 (.co(c23[41]),.s(s23[40]), .a(s14[40]),.b(c14[40]),.c(p16[08]));
fp_add3 U2341 (.co(c23[42]),.s(s23[41]), .a(s14[41]),.b(c14[41]),.c(p16[09]));
fp_add3 U2342 (.co(c23[43]),.s(s23[42]), .a(s14[42]),.b(c14[42]),.c(p16[10]));
fp_add3 U2343 (.co(c23[44]),.s(s23[43]), .a(s14[43]),.b(c14[43]),.c(p16[11]));
fp_add3 U2344 (.co(c23[45]),.s(s23[44]), .a(s14[44]),.b(c14[44]),.c(p16[12]));
fp_add3 U2345 (.co(c23[46]),.s(s23[45]), .a(s14[45]),.b(c14[45]),.c(p16[13]));
fp_add3 U2346 (.co(c23[47]),.s(s23[46]), .a(s14[46]),.b(c14[46]),.c(p16[14]));
fp_add3 U2347 (.co(c23[48]),.s(s23[47]), .a(s14[47]),.b(c14[47]),.c(p16[15]));
fp_add3 U2348 (.co(c23[49]),.s(s23[48]), .a(s14[48]),.b(c14[48]),.c(p16[16]));
fp_add3 U2349 (.co(c23[50]),.s(s23[49]), .a(s14[49]),.b(c14[49]),.c(p16[17]));
fp_add3 U2350 (.co(c23[51]),.s(s23[50]), .a(s14[50]),.b(c14[50]),.c(p16[18]));
fp_add3 U2351 (.co(c23[52]),.s(s23[51]), .a(s14[51]),.b(c14[51]),.c(p16[19]));
fp_add3 U2352 (.co(c23[53]),.s(s23[52]), .a(s14[52]),.b(c14[52]),.c(p16[20]));
fp_add3 U2353 (.co(c23[54]),.s(s23[53]), .a(s14[53]),.b(c14[53]),.c(p16[21]));
fp_add3 U2354 (.co(c23[55]),.s(s23[54]), .a(s14[54]),.b(c14[54]),.c(p16[22]));
fp_add3 U2355 (.co(c23[56]),.s(s23[55]), .a(s14[55]),.b(c14[55]),.c(p16[23]));
fp_add3 U2356 (.co(c23[57]),.s(s23[56]), .a(s14[56]),.b(c14[56]),.c(p16[24]));
fp_add3 U2357 (.co(c23[58]),.s(s23[57]), .a(s14[57]),.b(c14[57]),.c(p16[25]));
fp_add3 U2358 (.co(c23[59]),.s(s23[58]), .a(s14[58]),.b(c14[58]),.c(p16[26]));
fp_add3 U2359 (.co(c23[60]),.s(s23[59]), .a(s14[59]),.b(c14[59]),.c(p16[27]));
fp_add3 U2360 (.co(c23[61]),.s(s23[60]), .a(s14[60]),.b(c14[60]),.c(p16[28]));
fp_add3 U2361 (.co(c23[62]),.s(s23[61]), .a(s14[61]),.b(c14[61]),.c(p16[29]));
fp_add3 U2362 (.co(c23[63]),.s(s23[62]), .a(s14[62]),.b(c14[62]),.c(p16[30]));
fp_add3 U2363 (.co(c23[64]),.s(s23[63]), .a(s14[63]),.b(c14[63]),.c(p16[31]));
fp_add3 U2364 (.co(c23[65]),.s(s23[64]), .a(s14[64]),.b(  1'b0 ),.c(p16[32]));
buf     U2365 (                s23[65] ,                            q16_33  );
endmodule
module fpmcp2 (
   ms, mc,
   s20, c20_00, c20_03, c20, s21, c21, s22_16, s22, c22,
   s23_24, s23, c23, opc
);
output [74:00] ms;
output [75:01] mc;
input  [44:00] s20;
input          c20_00;
input          c20_03;
input  [39:05] c20;
input  [50:07] s21;
input  [50:11] c21;
input          s22_16;
input  [57:18] s22;
input  [57:20] c22;
input          s23_24;
input  [65:26] s23;
input  [65:28] c23;
input  [74:00] opc;
wire [50:00] s30;
wire [45:08] c30;
wire         c30_00;
wire         c30_03;
wire         c30_05;
wire         c30_06;
wire [58:11] s31;
wire [58:17] c31;
wire [58:00] s40;
wire [51:12] c40;
wire         c40_00;
wire         c40_03;
wire         c40_05;
wire         c40_06;
wire         c40_08;
wire         c40_09;
wire         c40_10;
wire [65:17] s41;
wire [66:25] c41;
wire [74:00] k41;
wire [65:00] s50;
wire [59:18] c50;
wire         c50_00;
wire         c50_03;
wire         c50_05;
wire         c50_06;
wire         c50_08;
wire         c50_09;
wire         c50_10;
wire         c50_12;
wire         c50_13;
wire         c50_14;
wire         c50_15;
wire         c50_16;
buf     U3000s(                s30[00],     s20[00]);
buf     U3001s(                s30[01],     s20[01]);
buf     U3002s(                s30[02],     s20[02]);
buf     U3003s(                s30[03],     s20[03]);
buf     U3004s(                s30[04],     s20[04]);
buf     U3005s(                s30[05],     s20[05]);
buf     U3006s(                s30[06],     s20[06]);
buf     U3000c(    c30_00  ,                            c20_00 );
buf     U3003c(    c30_03  ,                            c20_03 );
buf     U3005c(    c30_05  ,                            c20[05]);
buf     U3006c(    c30_06  ,                            c20[06]);
fp_add3 U3007 (.co(c30[08]),.s(s30[07]), .a(s20[07]),.b(c20[07]),.c(s21[07]));
fp_add3 U3008 (.co(c30[09]),.s(s30[08]), .a(s20[08]),.b(c20[08]),.c(s21[08]));
fp_add3 U3009 (.co(c30[10]),.s(s30[09]), .a(s20[09]),.b(c20[09]),.c(s21[09]));
fp_add3 U3010 (.co(c30[11]),.s(s30[10]), .a(s20[10]),.b(c20[10]),.c(s21[10]));
fp_add3 U3011 (.co(c30[12]),.s(s30[11]), .a(s20[11]),.b(c20[11]),.c(s21[11]));
fp_add3 U3012 (.co(c30[13]),.s(s30[12]), .a(s20[12]),.b(c20[12]),.c(s21[12]));
fp_add3 U3013 (.co(c30[14]),.s(s30[13]), .a(s20[13]),.b(c20[13]),.c(s21[13]));
fp_add3 U3014 (.co(c30[15]),.s(s30[14]), .a(s20[14]),.b(c20[14]),.c(s21[14]));
fp_add3 U3015 (.co(c30[16]),.s(s30[15]), .a(s20[15]),.b(c20[15]),.c(s21[15]));
fp_add3 U3016 (.co(c30[17]),.s(s30[16]), .a(s20[16]),.b(c20[16]),.c(s21[16]));
fp_add3 U3017 (.co(c30[18]),.s(s30[17]), .a(s20[17]),.b(c20[17]),.c(s21[17]));
fp_add3 U3018 (.co(c30[19]),.s(s30[18]), .a(s20[18]),.b(c20[18]),.c(s21[18]));
fp_add3 U3019 (.co(c30[20]),.s(s30[19]), .a(s20[19]),.b(c20[19]),.c(s21[19]));
fp_add3 U3020 (.co(c30[21]),.s(s30[20]), .a(s20[20]),.b(c20[20]),.c(s21[20]));
fp_add3 U3021 (.co(c30[22]),.s(s30[21]), .a(s20[21]),.b(c20[21]),.c(s21[21]));
fp_add3 U3022 (.co(c30[23]),.s(s30[22]), .a(s20[22]),.b(c20[22]),.c(s21[22]));
fp_add3 U3023 (.co(c30[24]),.s(s30[23]), .a(s20[23]),.b(c20[23]),.c(s21[23]));
fp_add3 U3024 (.co(c30[25]),.s(s30[24]), .a(s20[24]),.b(c20[24]),.c(s21[24]));
fp_add3 U3025 (.co(c30[26]),.s(s30[25]), .a(s20[25]),.b(c20[25]),.c(s21[25]));
fp_add3 U3026 (.co(c30[27]),.s(s30[26]), .a(s20[26]),.b(c20[26]),.c(s21[26]));
fp_add3 U3027 (.co(c30[28]),.s(s30[27]), .a(s20[27]),.b(c20[27]),.c(s21[27]));
fp_add3 U3028 (.co(c30[29]),.s(s30[28]), .a(s20[28]),.b(c20[28]),.c(s21[28]));
fp_add3 U3029 (.co(c30[30]),.s(s30[29]), .a(s20[29]),.b(c20[29]),.c(s21[29]));
fp_add3 U3030 (.co(c30[31]),.s(s30[30]), .a(s20[30]),.b(c20[30]),.c(s21[30]));
fp_add3 U3031 (.co(c30[32]),.s(s30[31]), .a(s20[31]),.b(c20[31]),.c(s21[31]));
fp_add3 U3032 (.co(c30[33]),.s(s30[32]), .a(s20[32]),.b(c20[32]),.c(s21[32]));
fp_add3 U3033 (.co(c30[34]),.s(s30[33]), .a(s20[33]),.b(c20[33]),.c(s21[33]));
fp_add3 U3034 (.co(c30[35]),.s(s30[34]), .a(s20[34]),.b(c20[34]),.c(s21[34]));
fp_add3 U3035 (.co(c30[36]),.s(s30[35]), .a(s20[35]),.b(c20[35]),.c(s21[35]));
fp_add3 U3036 (.co(c30[37]),.s(s30[36]), .a(s20[36]),.b(c20[36]),.c(s21[36]));
fp_add3 U3037 (.co(c30[38]),.s(s30[37]), .a(s20[37]),.b(c20[37]),.c(s21[37]));
fp_add3 U3038 (.co(c30[39]),.s(s30[38]), .a(s20[38]),.b(c20[38]),.c(s21[38]));
fp_add3 U3039 (.co(c30[40]),.s(s30[39]), .a(s20[39]),.b(c20[39]),.c(s21[39]));
fp_add3 U3040 (.co(c30[41]),.s(s30[40]), .a(s20[40]),.b(  1'b0 ),.c(s21[40]));
fp_add3 U3041 (.co(c30[42]),.s(s30[41]), .a(s20[41]),.b(  1'b0 ),.c(s21[41]));
fp_add3 U3042 (.co(c30[43]),.s(s30[42]), .a(s20[42]),.b(  1'b0 ),.c(s21[42]));
fp_add3 U3043 (.co(c30[44]),.s(s30[43]), .a(s20[43]),.b(  1'b0 ),.c(s21[43]));
fp_add3 U3044 (.co(c30[45]),.s(s30[44]), .a(s20[44]),.b(  1'b0 ),.c(s21[44]));
buf     U3045 (                s30[45],                             s21[45] );
buf     U3046 (                s30[46],                             s21[46] );
buf     U3047 (                s30[47],                             s21[47] );
buf     U3048 (                s30[48],                             s21[48] );
buf     U3049 (                s30[49],                             s21[49] );
buf     U3050 (                s30[50],                             s21[50] );
buf     U3111 (                s31[11] ,    c21[11]);
buf     U3112 (                s31[12] ,    c21[12]);
buf     U3113 (                s31[13] ,    c21[13]);
buf     U3114 (                s31[14] ,    c21[14]);
buf     U3115 (                s31[15] ,    c21[15]);
fp_add3 U3116 (.co(c31[17]),.s(s31[16]), .a(c21[16]),.b(s22_16 ),.c(  1'b0 ));
fp_add3 U3117 (.co(c31[18]),.s(s31[17]), .a(c21[17]),.b(  1'b0 ),.c(  1'b0 ));
fp_add3 U3118 (.co(c31[19]),.s(s31[18]), .a(c21[18]),.b(s22[18]),.c(  1'b0 ));
fp_add3 U3119 (.co(c31[20]),.s(s31[19]), .a(c21[19]),.b(s22[19]),.c(  1'b0 ));
fp_add3 U3120 (.co(c31[21]),.s(s31[20]), .a(c21[20]),.b(s22[20]),.c(c22[20]));
fp_add3 U3121 (.co(c31[22]),.s(s31[21]), .a(c21[21]),.b(s22[21]),.c(c22[21]));
fp_add3 U3122 (.co(c31[23]),.s(s31[22]), .a(c21[22]),.b(s22[22]),.c(c22[22]));
fp_add3 U3123 (.co(c31[24]),.s(s31[23]), .a(c21[23]),.b(s22[23]),.c(c22[23]));
fp_add3 U3124 (.co(c31[25]),.s(s31[24]), .a(c21[24]),.b(s22[24]),.c(c22[24]));
fp_add3 U3125 (.co(c31[26]),.s(s31[25]), .a(c21[25]),.b(s22[25]),.c(c22[25]));
fp_add3 U3126 (.co(c31[27]),.s(s31[26]), .a(c21[26]),.b(s22[26]),.c(c22[26]));
fp_add3 U3127 (.co(c31[28]),.s(s31[27]), .a(c21[27]),.b(s22[27]),.c(c22[27]));
fp_add3 U3128 (.co(c31[29]),.s(s31[28]), .a(c21[28]),.b(s22[28]),.c(c22[28]));
fp_add3 U3129 (.co(c31[30]),.s(s31[29]), .a(c21[29]),.b(s22[29]),.c(c22[29]));
fp_add3 U3130 (.co(c31[31]),.s(s31[30]), .a(c21[30]),.b(s22[30]),.c(c22[30]));
fp_add3 U3131 (.co(c31[32]),.s(s31[31]), .a(c21[31]),.b(s22[31]),.c(c22[31]));
fp_add3 U3132 (.co(c31[33]),.s(s31[32]), .a(c21[32]),.b(s22[32]),.c(c22[32]));
fp_add3 U3133 (.co(c31[34]),.s(s31[33]), .a(c21[33]),.b(s22[33]),.c(c22[33]));
fp_add3 U3134 (.co(c31[35]),.s(s31[34]), .a(c21[34]),.b(s22[34]),.c(c22[34]));
fp_add3 U3135 (.co(c31[36]),.s(s31[35]), .a(c21[35]),.b(s22[35]),.c(c22[35]));
fp_add3 U3136 (.co(c31[37]),.s(s31[36]), .a(c21[36]),.b(s22[36]),.c(c22[36]));
fp_add3 U3137 (.co(c31[38]),.s(s31[37]), .a(c21[37]),.b(s22[37]),.c(c22[37]));
fp_add3 U3138 (.co(c31[39]),.s(s31[38]), .a(c21[38]),.b(s22[38]),.c(c22[38]));
fp_add3 U3139 (.co(c31[40]),.s(s31[39]), .a(c21[39]),.b(s22[39]),.c(c22[39]));
fp_add3 U3140 (.co(c31[41]),.s(s31[40]), .a(c21[40]),.b(s22[40]),.c(c22[40]));
fp_add3 U3141 (.co(c31[42]),.s(s31[41]), .a(c21[41]),.b(s22[41]),.c(c22[41]));
fp_add3 U3142 (.co(c31[43]),.s(s31[42]), .a(c21[42]),.b(s22[42]),.c(c22[42]));
fp_add3 U3143 (.co(c31[44]),.s(s31[43]), .a(c21[43]),.b(s22[43]),.c(c22[43]));
fp_add3 U3144 (.co(c31[45]),.s(s31[44]), .a(c21[44]),.b(s22[44]),.c(c22[44]));
fp_add3 U3145 (.co(c31[46]),.s(s31[45]), .a(c21[45]),.b(s22[45]),.c(c22[45]));
fp_add3 U3146 (.co(c31[47]),.s(s31[46]), .a(c21[46]),.b(s22[46]),.c(c22[46]));
fp_add3 U3147 (.co(c31[48]),.s(s31[47]), .a(c21[47]),.b(s22[47]),.c(c22[47]));
fp_add3 U3148 (.co(c31[49]),.s(s31[48]), .a(c21[48]),.b(s22[48]),.c(c22[48]));
fp_add3 U3149 (.co(c31[50]),.s(s31[49]), .a(c21[49]),.b(s22[49]),.c(c22[49]));
fp_add3 U3150 (.co(c31[51]),.s(s31[50]), .a(c21[50]),.b(s22[50]),.c(c22[50]));
fp_add3 U3151 (.co(c31[52]),.s(s31[51]), .a(  1'b0 ),.b(s22[51]),.c(c22[51]));
fp_add3 U3152 (.co(c31[53]),.s(s31[52]), .a(  1'b0 ),.b(s22[52]),.c(c22[52]));
fp_add3 U3153 (.co(c31[54]),.s(s31[53]), .a(  1'b0 ),.b(s22[53]),.c(c22[53]));
fp_add3 U3154 (.co(c31[55]),.s(s31[54]), .a(  1'b0 ),.b(s22[54]),.c(c22[54]));
fp_add3 U3155 (.co(c31[56]),.s(s31[55]), .a(  1'b0 ),.b(s22[55]),.c(c22[55]));
fp_add3 U3156 (.co(c31[57]),.s(s31[56]), .a(  1'b0 ),.b(s22[56]),.c(c22[56]));
fp_add3 U3157 (.co(c31[58]),.s(s31[57]), .a(  1'b0 ),.b(s22[57]),.c(c22[57]));
buf     U3158 (                s31[58] ,                  1'b1 );
buf     U4000s(                s40[00],     s30[00]);
buf     U4001s(                s40[01],     s30[01]);
buf     U4002s(                s40[02],     s30[02]);
buf     U4003s(                s40[03],     s30[03]);
buf     U4004s(                s40[04],     s30[04]);
buf     U4005s(                s40[05],     s30[05]);
buf     U4006s(                s40[06],     s30[06]);
buf     U4007s(                s40[07],     s30[07]);
buf     U4008s(                s40[08],     s30[08]);
buf     U4009s(                s40[09],     s30[09]);
buf     U4010s(                s40[10],     s30[10]);
buf     U4000c(    c40_00  ,                            c30_00 );
buf     U4003c(    c40_03  ,                            c30_03 );
buf     U4005c(    c40_05  ,                            c30_05 );
buf     U4006c(    c40_06  ,                            c30_06 );
buf     U4008c(    c40_08  ,                            c30[08]);
buf     U4009c(    c40_09  ,                            c30[09]);
buf     U4010c(    c40_10  ,                            c30[10]);
fp_add3 U4011 (.co(c40[12]),.s(s40[11]), .a(s30[11]),.b(c30[11]),.c(s31[11]));
fp_add3 U4012 (.co(c40[13]),.s(s40[12]), .a(s30[12]),.b(c30[12]),.c(s31[12]));
fp_add3 U4013 (.co(c40[14]),.s(s40[13]), .a(s30[13]),.b(c30[13]),.c(s31[13]));
fp_add3 U4014 (.co(c40[15]),.s(s40[14]), .a(s30[14]),.b(c30[14]),.c(s31[14]));
fp_add3 U4015 (.co(c40[16]),.s(s40[15]), .a(s30[15]),.b(c30[15]),.c(s31[15]));
fp_add3 U4016 (.co(c40[17]),.s(s40[16]), .a(s30[16]),.b(c30[16]),.c(s31[16]));
fp_add3 U4017 (.co(c40[18]),.s(s40[17]), .a(s30[17]),.b(c30[17]),.c(s31[17]));
fp_add3 U4018 (.co(c40[19]),.s(s40[18]), .a(s30[18]),.b(c30[18]),.c(s31[18]));
fp_add3 U4019 (.co(c40[20]),.s(s40[19]), .a(s30[19]),.b(c30[19]),.c(s31[19]));
fp_add3 U4020 (.co(c40[21]),.s(s40[20]), .a(s30[20]),.b(c30[20]),.c(s31[20]));
fp_add3 U4021 (.co(c40[22]),.s(s40[21]), .a(s30[21]),.b(c30[21]),.c(s31[21]));
fp_add3 U4022 (.co(c40[23]),.s(s40[22]), .a(s30[22]),.b(c30[22]),.c(s31[22]));
fp_add3 U4023 (.co(c40[24]),.s(s40[23]), .a(s30[23]),.b(c30[23]),.c(s31[23]));
fp_add3 U4024 (.co(c40[25]),.s(s40[24]), .a(s30[24]),.b(c30[24]),.c(s31[24]));
fp_add3 U4025 (.co(c40[26]),.s(s40[25]), .a(s30[25]),.b(c30[25]),.c(s31[25]));
fp_add3 U4026 (.co(c40[27]),.s(s40[26]), .a(s30[26]),.b(c30[26]),.c(s31[26]));
fp_add3 U4027 (.co(c40[28]),.s(s40[27]), .a(s30[27]),.b(c30[27]),.c(s31[27]));
fp_add3 U4028 (.co(c40[29]),.s(s40[28]), .a(s30[28]),.b(c30[28]),.c(s31[28]));
fp_add3 U4029 (.co(c40[30]),.s(s40[29]), .a(s30[29]),.b(c30[29]),.c(s31[29]));
fp_add3 U4030 (.co(c40[31]),.s(s40[30]), .a(s30[30]),.b(c30[30]),.c(s31[30]));
fp_add3 U4031 (.co(c40[32]),.s(s40[31]), .a(s30[31]),.b(c30[31]),.c(s31[31]));
fp_add3 U4032 (.co(c40[33]),.s(s40[32]), .a(s30[32]),.b(c30[32]),.c(s31[32]));
fp_add3 U4033 (.co(c40[34]),.s(s40[33]), .a(s30[33]),.b(c30[33]),.c(s31[33]));
fp_add3 U4034 (.co(c40[35]),.s(s40[34]), .a(s30[34]),.b(c30[34]),.c(s31[34]));
fp_add3 U4035 (.co(c40[36]),.s(s40[35]), .a(s30[35]),.b(c30[35]),.c(s31[35]));
fp_add3 U4036 (.co(c40[37]),.s(s40[36]), .a(s30[36]),.b(c30[36]),.c(s31[36]));
fp_add3 U4037 (.co(c40[38]),.s(s40[37]), .a(s30[37]),.b(c30[37]),.c(s31[37]));
fp_add3 U4038 (.co(c40[39]),.s(s40[38]), .a(s30[38]),.b(c30[38]),.c(s31[38]));
fp_add3 U4039 (.co(c40[40]),.s(s40[39]), .a(s30[39]),.b(c30[39]),.c(s31[39]));
fp_add3 U4040 (.co(c40[41]),.s(s40[40]), .a(s30[40]),.b(c30[40]),.c(s31[40]));
fp_add3 U4041 (.co(c40[42]),.s(s40[41]), .a(s30[41]),.b(c30[41]),.c(s31[41]));
fp_add3 U4042 (.co(c40[43]),.s(s40[42]), .a(s30[42]),.b(c30[42]),.c(s31[42]));
fp_add3 U4043 (.co(c40[44]),.s(s40[43]), .a(s30[43]),.b(c30[43]),.c(s31[43]));
fp_add3 U4044 (.co(c40[45]),.s(s40[44]), .a(s30[44]),.b(c30[44]),.c(s31[44]));
fp_add3 U4045 (.co(c40[46]),.s(s40[45]), .a(s30[45]),.b(c30[45]),.c(s31[45]));
fp_add3 U4046 (.co(c40[47]),.s(s40[46]), .a(s30[46]),.b(  1'b0 ),.c(s31[46]));
fp_add3 U4047 (.co(c40[48]),.s(s40[47]), .a(s30[47]),.b(  1'b0 ),.c(s31[47]));
fp_add3 U4048 (.co(c40[49]),.s(s40[48]), .a(s30[48]),.b(  1'b0 ),.c(s31[48]));
fp_add3 U4049 (.co(c40[50]),.s(s40[49]), .a(s30[49]),.b(  1'b0 ),.c(s31[49]));
fp_add3 U4050 (.co(c40[51]),.s(s40[50]), .a(s30[50]),.b(  1'b0 ),.c(s31[50]));
buf     U4051 (                s40[51] ,                            s31[51]);
buf     U4052 (                s40[52] ,                            s31[52]);
buf     U4053 (                s40[53] ,                            s31[53]);
buf     U4054 (                s40[54] ,                            s31[54]);
buf     U4055 (                s40[55] ,                            s31[55]);
buf     U4056 (                s40[56] ,                            s31[56]);
buf     U4057 (                s40[57] ,                            s31[57]);
buf     U4058 (                s40[58] ,                            s31[58]);
buf     U4117 (                s41[17],     c31[17]);
buf     U4118 (                s41[18],     c31[18]);
buf     U4119 (                s41[19],     c31[19]);
buf     U4120 (                s41[20],     c31[20]);
buf     U4121 (                s41[21],     c31[21]);
buf     U4122 (                s41[22],     c31[22]);
buf     U4123 (                s41[23],     c31[23]);
fp_add3 U4124 (.co(c41[25]),.s(s41[24]), .a(c31[24]),.b(s23_24 ),.c(  1'b0 ));
fp_add3 U4125 (.co(c41[26]),.s(s41[25]), .a(c31[25]),.b(  1'b0 ),.c(  1'b0 ));
fp_add3 U4126 (.co(c41[27]),.s(s41[26]), .a(c31[26]),.b(s23[26]),.c(  1'b0 ));
fp_add3 U4127 (.co(c41[28]),.s(s41[27]), .a(c31[27]),.b(s23[27]),.c(  1'b0 ));
fp_add3 U4128 (.co(c41[29]),.s(s41[28]), .a(c31[28]),.b(s23[28]),.c(c23[28]));
fp_add3 U4129 (.co(c41[30]),.s(s41[29]), .a(c31[29]),.b(s23[29]),.c(c23[29]));
fp_add3 U4130 (.co(c41[31]),.s(s41[30]), .a(c31[30]),.b(s23[30]),.c(c23[30]));
fp_add3 U4131 (.co(c41[32]),.s(s41[31]), .a(c31[31]),.b(s23[31]),.c(c23[31]));
fp_add3 U4132 (.co(c41[33]),.s(s41[32]), .a(c31[32]),.b(s23[32]),.c(c23[32]));
fp_add3 U4133 (.co(c41[34]),.s(s41[33]), .a(c31[33]),.b(s23[33]),.c(c23[33]));
fp_add3 U4134 (.co(c41[35]),.s(s41[34]), .a(c31[34]),.b(s23[34]),.c(c23[34]));
fp_add3 U4135 (.co(c41[36]),.s(s41[35]), .a(c31[35]),.b(s23[35]),.c(c23[35]));
fp_add3 U4136 (.co(c41[37]),.s(s41[36]), .a(c31[36]),.b(s23[36]),.c(c23[36]));
fp_add3 U4137 (.co(c41[38]),.s(s41[37]), .a(c31[37]),.b(s23[37]),.c(c23[37]));
fp_add3 U4138 (.co(c41[39]),.s(s41[38]), .a(c31[38]),.b(s23[38]),.c(c23[38]));
fp_add3 U4139 (.co(c41[40]),.s(s41[39]), .a(c31[39]),.b(s23[39]),.c(c23[39]));
fp_add3 U4140 (.co(c41[41]),.s(s41[40]), .a(c31[40]),.b(s23[40]),.c(c23[40]));
fp_add3 U4141 (.co(c41[42]),.s(s41[41]), .a(c31[41]),.b(s23[41]),.c(c23[41]));
fp_add3 U4142 (.co(c41[43]),.s(s41[42]), .a(c31[42]),.b(s23[42]),.c(c23[42]));
fp_add3 U4143 (.co(c41[44]),.s(s41[43]), .a(c31[43]),.b(s23[43]),.c(c23[43]));
fp_add3 U4144 (.co(c41[45]),.s(s41[44]), .a(c31[44]),.b(s23[44]),.c(c23[44]));
fp_add3 U4145 (.co(c41[46]),.s(s41[45]), .a(c31[45]),.b(s23[45]),.c(c23[45]));
fp_add3 U4146 (.co(c41[47]),.s(s41[46]), .a(c31[46]),.b(s23[46]),.c(c23[46]));
fp_add3 U4147 (.co(c41[48]),.s(s41[47]), .a(c31[47]),.b(s23[47]),.c(c23[47]));
fp_add3 U4148 (.co(c41[49]),.s(s41[48]), .a(c31[48]),.b(s23[48]),.c(c23[48]));
fp_add3 U4149 (.co(c41[50]),.s(s41[49]), .a(c31[49]),.b(s23[49]),.c(c23[49]));
fp_add3 U4150 (.co(c41[51]),.s(s41[50]), .a(c31[50]),.b(s23[50]),.c(c23[50]));
fp_add3 U4151 (.co(c41[52]),.s(s41[51]), .a(c31[51]),.b(s23[51]),.c(c23[51]));
fp_add3 U4152 (.co(c41[53]),.s(s41[52]), .a(c31[52]),.b(s23[52]),.c(c23[52]));
fp_add3 U4153 (.co(c41[54]),.s(s41[53]), .a(c31[53]),.b(s23[53]),.c(c23[53]));
fp_add3 U4154 (.co(c41[55]),.s(s41[54]), .a(c31[54]),.b(s23[54]),.c(c23[54]));
fp_add3 U4155 (.co(c41[56]),.s(s41[55]), .a(c31[55]),.b(s23[55]),.c(c23[55]));
fp_add3 U4156 (.co(c41[57]),.s(s41[56]), .a(c31[56]),.b(s23[56]),.c(c23[56]));
fp_add3 U4157 (.co(c41[58]),.s(s41[57]), .a(c31[57]),.b(s23[57]),.c(c23[57]));
fp_add3 U4158 (.co(c41[59]),.s(s41[58]), .a(c31[58]),.b(s23[58]),.c(c23[58]));
fp_add3 U4159 (.co(c41[60]),.s(s41[59]), .a(  1'b0 ),.b(s23[59]),.c(c23[59]));
fp_add3 U4160 (.co(c41[61]),.s(s41[60]), .a(  1'b0 ),.b(s23[60]),.c(c23[60]));
fp_add3 U4161 (.co(c41[62]),.s(s41[61]), .a(  1'b0 ),.b(s23[61]),.c(c23[61]));
fp_add3 U4162 (.co(c41[63]),.s(s41[62]), .a(  1'b0 ),.b(s23[62]),.c(c23[62]));
fp_add3 U4163 (.co(c41[64]),.s(s41[63]), .a(  1'b0 ),.b(s23[63]),.c(c23[63]));
fp_add3 U4164 (.co(c41[65]),.s(s41[64]), .a(  1'b0 ),.b(s23[64]),.c(c23[64]));
fp_add3 U4165 (.co(c41[66]),.s(s41[65]), .a(  1'b0 ),.b(s23[65]),.c(c23[65]));
buf     U5000s(                s50[00],     s40[00]);
buf     U5001s(                s50[01],     s40[01]);
buf     U5002s(                s50[02],     s40[02]);
buf     U5003s(                s50[03],     s40[03]);
buf     U5004s(                s50[04],     s40[04]);
buf     U5005s(                s50[05],     s40[05]);
buf     U5006s(                s50[06],     s40[06]);
buf     U5007s(                s50[07],     s40[07]);
buf     U5008s(                s50[08],     s40[08]);
buf     U5009s(                s50[09],     s40[09]);
buf     U5010s(                s50[10],     s40[10]);
buf     U5011s(                s50[11],     s40[11]);
buf     U5012s(                s50[12],     s40[12]);
buf     U5013s(                s50[13],     s40[13]);
buf     U5014s(                s50[14],     s40[14]);
buf     U5015s(                s50[15],     s40[15]);
buf     U5016s(                s50[16],     s40[16]);
buf     U5000c(    c50_00  ,                            c40_00 );
buf     U5003c(    c50_03  ,                            c40_03 );
buf     U5005c(    c50_05  ,                            c40_05 );
buf     U5006c(    c50_06  ,                            c40_06 );
buf     U5008c(    c50_08  ,                            c40_08 );
buf     U5009c(    c50_09  ,                            c40_09 );
buf     U5010c(    c50_10  ,                            c40_10 );
buf     U5012c(    c50_12  ,                            c40[12]);
buf     U5013c(    c50_13  ,                            c40[13]);
buf     U5014c(    c50_14  ,                            c40[14]);
buf     U5015c(    c50_15  ,                            c40[15]);
buf     U5016c(    c50_16  ,                            c40[16]);
fp_add3 U5017 (.co(c50[18]),.s(s50[17]), .a(s40[17]),.b(c40[17]),.c(s41[17]));
fp_add3 U5018 (.co(c50[19]),.s(s50[18]), .a(s40[18]),.b(c40[18]),.c(s41[18]));
fp_add3 U5019 (.co(c50[20]),.s(s50[19]), .a(s40[19]),.b(c40[19]),.c(s41[19]));
fp_add3 U5020 (.co(c50[21]),.s(s50[20]), .a(s40[20]),.b(c40[20]),.c(s41[20]));
fp_add3 U5021 (.co(c50[22]),.s(s50[21]), .a(s40[21]),.b(c40[21]),.c(s41[21]));
fp_add3 U5022 (.co(c50[23]),.s(s50[22]), .a(s40[22]),.b(c40[22]),.c(s41[22]));
fp_add3 U5023 (.co(c50[24]),.s(s50[23]), .a(s40[23]),.b(c40[23]),.c(s41[23]));
fp_add3 U5024 (.co(c50[25]),.s(s50[24]), .a(s40[24]),.b(c40[24]),.c(s41[24]));
fp_add3 U5025 (.co(c50[26]),.s(s50[25]), .a(s40[25]),.b(c40[25]),.c(s41[25]));
fp_add3 U5026 (.co(c50[27]),.s(s50[26]), .a(s40[26]),.b(c40[26]),.c(s41[26]));
fp_add3 U5027 (.co(c50[28]),.s(s50[27]), .a(s40[27]),.b(c40[27]),.c(s41[27]));
fp_add3 U5028 (.co(c50[29]),.s(s50[28]), .a(s40[28]),.b(c40[28]),.c(s41[28]));
fp_add3 U5029 (.co(c50[30]),.s(s50[29]), .a(s40[29]),.b(c40[29]),.c(s41[29]));
fp_add3 U5030 (.co(c50[31]),.s(s50[30]), .a(s40[30]),.b(c40[30]),.c(s41[30]));
fp_add3 U5031 (.co(c50[32]),.s(s50[31]), .a(s40[31]),.b(c40[31]),.c(s41[31]));
fp_add3 U5032 (.co(c50[33]),.s(s50[32]), .a(s40[32]),.b(c40[32]),.c(s41[32]));
fp_add3 U5033 (.co(c50[34]),.s(s50[33]), .a(s40[33]),.b(c40[33]),.c(s41[33]));
fp_add3 U5034 (.co(c50[35]),.s(s50[34]), .a(s40[34]),.b(c40[34]),.c(s41[34]));
fp_add3 U5035 (.co(c50[36]),.s(s50[35]), .a(s40[35]),.b(c40[35]),.c(s41[35]));
fp_add3 U5036 (.co(c50[37]),.s(s50[36]), .a(s40[36]),.b(c40[36]),.c(s41[36]));
fp_add3 U5037 (.co(c50[38]),.s(s50[37]), .a(s40[37]),.b(c40[37]),.c(s41[37]));
fp_add3 U5038 (.co(c50[39]),.s(s50[38]), .a(s40[38]),.b(c40[38]),.c(s41[38]));
fp_add3 U5039 (.co(c50[40]),.s(s50[39]), .a(s40[39]),.b(c40[39]),.c(s41[39]));
fp_add3 U5040 (.co(c50[41]),.s(s50[40]), .a(s40[40]),.b(c40[40]),.c(s41[40]));
fp_add3 U5041 (.co(c50[42]),.s(s50[41]), .a(s40[41]),.b(c40[41]),.c(s41[41]));
fp_add3 U5042 (.co(c50[43]),.s(s50[42]), .a(s40[42]),.b(c40[42]),.c(s41[42]));
fp_add3 U5043 (.co(c50[44]),.s(s50[43]), .a(s40[43]),.b(c40[43]),.c(s41[43]));
fp_add3 U5044 (.co(c50[45]),.s(s50[44]), .a(s40[44]),.b(c40[44]),.c(s41[44]));
fp_add3 U5045 (.co(c50[46]),.s(s50[45]), .a(s40[45]),.b(c40[45]),.c(s41[45]));
fp_add3 U5046 (.co(c50[47]),.s(s50[46]), .a(s40[46]),.b(c40[46]),.c(s41[46]));
fp_add3 U5047 (.co(c50[48]),.s(s50[47]), .a(s40[47]),.b(c40[47]),.c(s41[47]));
fp_add3 U5048 (.co(c50[49]),.s(s50[48]), .a(s40[48]),.b(c40[48]),.c(s41[48]));
fp_add3 U5049 (.co(c50[50]),.s(s50[49]), .a(s40[49]),.b(c40[49]),.c(s41[49]));
fp_add3 U5050 (.co(c50[51]),.s(s50[50]), .a(s40[50]),.b(c40[50]),.c(s41[50]));
fp_add3 U5051 (.co(c50[52]),.s(s50[51]), .a(s40[51]),.b(c40[51]),.c(s41[51]));
fp_add3 U5052 (.co(c50[53]),.s(s50[52]), .a(s40[52]),.b(  1'b0 ),.c(s41[52]));
fp_add3 U5053 (.co(c50[54]),.s(s50[53]), .a(s40[53]),.b(  1'b0 ),.c(s41[53]));
fp_add3 U5054 (.co(c50[55]),.s(s50[54]), .a(s40[54]),.b(  1'b0 ),.c(s41[54]));
fp_add3 U5055 (.co(c50[56]),.s(s50[55]), .a(s40[55]),.b(  1'b0 ),.c(s41[55]));
fp_add3 U5056 (.co(c50[57]),.s(s50[56]), .a(s40[56]),.b(  1'b0 ),.c(s41[56]));
fp_add3 U5057 (.co(c50[58]),.s(s50[57]), .a(s40[57]),.b(  1'b0 ),.c(s41[57]));
fp_add3 U5058 (.co(c50[59]),.s(s50[58]), .a(s40[58]),.b(  1'b0 ),.c(s41[58]));
buf     U5059 (                s50[59] ,                            s41[59]);
buf     U5060 (                s50[60] ,                            s41[60]);
buf     U5061 (                s50[61] ,                            s41[61]);
buf     U5062 (                s50[62] ,                            s41[62]);
buf     U5063 (                s50[63] ,                            s41[63]);
buf     U5064 (                s50[64] ,                            s41[64]);
buf     U5065 (                s50[65] ,                            s41[65]);
assign k41[00] = opc[00];
assign k41[01] = opc[01];
assign k41[02] = opc[02];
assign k41[03] = opc[03];
assign k41[04] = opc[04];
assign k41[05] = opc[05];
assign k41[06] = opc[06];
assign k41[07] = opc[07];
assign k41[08] = opc[08];
assign k41[09] = opc[09];
assign k41[10] = opc[10];
assign k41[11] = opc[11];
assign k41[12] = opc[12];
assign k41[13] = opc[13];
assign k41[14] = opc[14];
assign k41[15] = opc[15];
assign k41[16] = opc[16];
assign k41[17] = opc[17];
assign k41[18] = opc[18];
assign k41[19] = opc[19];
assign k41[20] = opc[20];
assign k41[21] = opc[21];
assign k41[22] = opc[22];
assign k41[23] = opc[23];
assign k41[24] = opc[24];
assign k41[25] = opc[25];
assign k41[26] = opc[26] | c41[25];
assign k41[27] = opc[27] | c41[26];
assign k41[28] = opc[28] | c41[27];
assign k41[29] = opc[29] | c41[28];
assign k41[30] = opc[30] | c41[29];
assign k41[31] = opc[31] | c41[30];
assign k41[32] = opc[32] | c41[31];
assign k41[33] = opc[33] | c41[32];
assign k41[34] = opc[34] | c41[33];
assign k41[35] = opc[35] | c41[34];
assign k41[36] = opc[36] | c41[35];
assign k41[37] = opc[37] | c41[36];
assign k41[38] = opc[38] | c41[37];
assign k41[39] = opc[39] | c41[38];
assign k41[40] = opc[40] | c41[39];
assign k41[41] = opc[41] | c41[40];
assign k41[42] = opc[42] | c41[41];
assign k41[43] = opc[43] | c41[42];
assign k41[44] = opc[44] | c41[43];
assign k41[45] = opc[45] | c41[44];
assign k41[46] = opc[46] | c41[45];
assign k41[47] = opc[47] | c41[46];
assign k41[48] = opc[48] | c41[47];
assign k41[49] = opc[49] | c41[48];
assign k41[50] = opc[50] | c41[49];
assign k41[51] = opc[51] | c41[50];
assign k41[52] = opc[52] | c41[51];
assign k41[53] = opc[53] | c41[52];
assign k41[54] = opc[54] | c41[53];
assign k41[55] = opc[55] | c41[54];
assign k41[56] = opc[56] | c41[55];
assign k41[57] = opc[57] | c41[56];
assign k41[58] = opc[58] | c41[57];
assign k41[59] = opc[59] | c41[58];
assign k41[60] = opc[60] | c41[59];
assign k41[61] = opc[61] | c41[60];
assign k41[62] = opc[62] | c41[61];
assign k41[63] = opc[63] | c41[62];
assign k41[64] = opc[64] | c41[63];
assign k41[65] = opc[65] | c41[64];
assign k41[66] = opc[66] | c41[65];
assign k41[67] = opc[67] | c41[66];
assign k41[68] = opc[68];
assign k41[69] = opc[69];
assign k41[70] = opc[70];
assign k41[71] = opc[71];
assign k41[72] = opc[72];
assign k41[73] = opc[73];
assign k41[74] = opc[74];
buf     u6000s(               ms[00] ,                            k41[00] );
buf     u6000c(    mc[01] ,                                       k41[01] );
fp_add3 U6001 (.co(mc[02]),.s(ms[01]), .a(s50[00]),.b(c50_00 ),.c(  1'b0 ));
fp_add3 U6002 (.co(mc[03]),.s(ms[02]), .a(s50[01]),.b(  1'b0 ),.c(k41[02]));
fp_add3 U6003 (.co(mc[04]),.s(ms[03]), .a(s50[02]),.b(  1'b0 ),.c(k41[03]));
fp_add3 U6004 (.co(mc[05]),.s(ms[04]), .a(s50[03]),.b(c50_03 ),.c(k41[04]));
fp_add3 U6005 (.co(mc[06]),.s(ms[05]), .a(s50[04]),.b(  1'b0 ),.c(k41[05]));
fp_add3 U6006 (.co(mc[07]),.s(ms[06]), .a(s50[05]),.b(c50_05 ),.c(k41[06]));
fp_add3 U6007 (.co(mc[08]),.s(ms[07]), .a(s50[06]),.b(c50_06 ),.c(k41[07]));
fp_add3 U6008 (.co(mc[09]),.s(ms[08]), .a(s50[07]),.b(  1'b0 ),.c(k41[08]));
fp_add3 U6009 (.co(mc[10]),.s(ms[09]), .a(s50[08]),.b(c50_08 ),.c(k41[09]));
fp_add3 U6010 (.co(mc[11]),.s(ms[10]), .a(s50[09]),.b(c50_09 ),.c(k41[10]));
fp_add3 U6011 (.co(mc[12]),.s(ms[11]), .a(s50[10]),.b(c50_10 ),.c(k41[11]));
fp_add3 U6012 (.co(mc[13]),.s(ms[12]), .a(s50[11]),.b(  1'b0 ),.c(k41[12]));
fp_add3 U6013 (.co(mc[14]),.s(ms[13]), .a(s50[12]),.b(c50_12 ),.c(k41[13]));
fp_add3 U6014 (.co(mc[15]),.s(ms[14]), .a(s50[13]),.b(c50_13 ),.c(k41[14]));
fp_add3 U6015 (.co(mc[16]),.s(ms[15]), .a(s50[14]),.b(c50_14 ),.c(k41[15]));
fp_add3 U6016 (.co(mc[17]),.s(ms[16]), .a(s50[15]),.b(c50_15 ),.c(k41[16]));
fp_add3 U6017 (.co(mc[18]),.s(ms[17]), .a(s50[16]),.b(c50_16 ),.c(k41[17]));
fp_add3 U6018 (.co(mc[19]),.s(ms[18]), .a(s50[17]),.b(  1'b0 ),.c(k41[18]));
fp_add3 U6019 (.co(mc[20]),.s(ms[19]), .a(s50[18]),.b(c50[18]),.c(k41[19]));
fp_add3 U6020 (.co(mc[21]),.s(ms[20]), .a(s50[19]),.b(c50[19]),.c(k41[20]));
fp_add3 U6021 (.co(mc[22]),.s(ms[21]), .a(s50[20]),.b(c50[20]),.c(k41[21]));
fp_add3 U6022 (.co(mc[23]),.s(ms[22]), .a(s50[21]),.b(c50[21]),.c(k41[22]));
fp_add3 U6023 (.co(mc[24]),.s(ms[23]), .a(s50[22]),.b(c50[22]),.c(k41[23]));
fp_add3 U6024 (.co(mc[25]),.s(ms[24]), .a(s50[23]),.b(c50[23]),.c(k41[24]));
fp_add3 U6025 (.co(mc[26]),.s(ms[25]), .a(s50[24]),.b(c50[24]),.c(k41[25]));
fp_add3 U6026 (.co(mc[27]),.s(ms[26]), .a(s50[25]),.b(c50[25]),.c(k41[26]));
fp_add3 U6027 (.co(mc[28]),.s(ms[27]), .a(s50[26]),.b(c50[26]),.c(k41[27]));
fp_add3 U6028 (.co(mc[29]),.s(ms[28]), .a(s50[27]),.b(c50[27]),.c(k41[28]));
fp_add3 U6029 (.co(mc[30]),.s(ms[29]), .a(s50[28]),.b(c50[28]),.c(k41[29]));
fp_add3 U6030 (.co(mc[31]),.s(ms[30]), .a(s50[29]),.b(c50[29]),.c(k41[30]));
fp_add3 U6031 (.co(mc[32]),.s(ms[31]), .a(s50[30]),.b(c50[30]),.c(k41[31]));
fp_add3 U6032 (.co(mc[33]),.s(ms[32]), .a(s50[31]),.b(c50[31]),.c(k41[32]));
fp_add3 U6033 (.co(mc[34]),.s(ms[33]), .a(s50[32]),.b(c50[32]),.c(k41[33]));
fp_add3 U6034 (.co(mc[35]),.s(ms[34]), .a(s50[33]),.b(c50[33]),.c(k41[34]));
fp_add3 U6035 (.co(mc[36]),.s(ms[35]), .a(s50[34]),.b(c50[34]),.c(k41[35]));
fp_add3 U6036 (.co(mc[37]),.s(ms[36]), .a(s50[35]),.b(c50[35]),.c(k41[36]));
fp_add3 U6037 (.co(mc[38]),.s(ms[37]), .a(s50[36]),.b(c50[36]),.c(k41[37]));
fp_add3 U6038 (.co(mc[39]),.s(ms[38]), .a(s50[37]),.b(c50[37]),.c(k41[38]));
fp_add3 U6039 (.co(mc[40]),.s(ms[39]), .a(s50[38]),.b(c50[38]),.c(k41[39]));
fp_add3 U6040 (.co(mc[41]),.s(ms[40]), .a(s50[39]),.b(c50[39]),.c(k41[40]));
fp_add3 U6041 (.co(mc[42]),.s(ms[41]), .a(s50[40]),.b(c50[40]),.c(k41[41]));
fp_add3 U6042 (.co(mc[43]),.s(ms[42]), .a(s50[41]),.b(c50[41]),.c(k41[42]));
fp_add3 U6043 (.co(mc[44]),.s(ms[43]), .a(s50[42]),.b(c50[42]),.c(k41[43]));
fp_add3 U6044 (.co(mc[45]),.s(ms[44]), .a(s50[43]),.b(c50[43]),.c(k41[44]));
fp_add3 U6045 (.co(mc[46]),.s(ms[45]), .a(s50[44]),.b(c50[44]),.c(k41[45]));
fp_add3 U6046 (.co(mc[47]),.s(ms[46]), .a(s50[45]),.b(c50[45]),.c(k41[46]));
fp_add3 U6047 (.co(mc[48]),.s(ms[47]), .a(s50[46]),.b(c50[46]),.c(k41[47]));
fp_add3 U6048 (.co(mc[49]),.s(ms[48]), .a(s50[47]),.b(c50[47]),.c(k41[48]));
fp_add3 U6049 (.co(mc[50]),.s(ms[49]), .a(s50[48]),.b(c50[48]),.c(k41[49]));
fp_add3 U6050 (.co(mc[51]),.s(ms[50]), .a(s50[49]),.b(c50[49]),.c(k41[50]));
fp_add3 U6051 (.co(mc[52]),.s(ms[51]), .a(s50[50]),.b(c50[50]),.c(k41[51]));
fp_add3 U6052 (.co(mc[53]),.s(ms[52]), .a(s50[51]),.b(c50[51]),.c(k41[52]));
fp_add3 U6053 (.co(mc[54]),.s(ms[53]), .a(s50[52]),.b(c50[52]),.c(k41[53]));
fp_add3 U6054 (.co(mc[55]),.s(ms[54]), .a(s50[53]),.b(c50[53]),.c(k41[54]));
fp_add3 U6055 (.co(mc[56]),.s(ms[55]), .a(s50[54]),.b(c50[54]),.c(k41[55]));
fp_add3 U6056 (.co(mc[57]),.s(ms[56]), .a(s50[55]),.b(c50[55]),.c(k41[56]));
fp_add3 U6057 (.co(mc[58]),.s(ms[57]), .a(s50[56]),.b(c50[56]),.c(k41[57]));
fp_add3 U6058 (.co(mc[59]),.s(ms[58]), .a(s50[57]),.b(c50[57]),.c(k41[58]));
fp_add3 U6059 (.co(mc[60]),.s(ms[59]), .a(s50[58]),.b(c50[58]),.c(k41[59]));
fp_add3 U6060 (.co(mc[61]),.s(ms[60]), .a(s50[59]),.b(c50[59]),.c(k41[60]));
fp_add3 U6061 (.co(mc[62]),.s(ms[61]), .a(s50[60]),.b(  1'b0 ),.c(k41[61]));
fp_add3 U6062 (.co(mc[63]),.s(ms[62]), .a(s50[61]),.b(  1'b0 ),.c(k41[62]));
fp_add3 U6063 (.co(mc[64]),.s(ms[63]), .a(s50[62]),.b(  1'b0 ),.c(k41[63]));
fp_add3 U6064 (.co(mc[65]),.s(ms[64]), .a(s50[63]),.b(  1'b0 ),.c(k41[64]));
fp_add3 U6065 (.co(mc[66]),.s(ms[65]), .a(s50[64]),.b(  1'b0 ),.c(k41[65]));
fp_add3 U6066 (.co(mc[67]),.s(ms[66]), .a(s50[65]),.b(  1'b0 ),.c(k41[66]));
not     U6067s(               ms[67] ,                            k41[67]);
buf     U6067c(    mc[68] ,                                       k41[67]);
not     U6068s(               ms[68] ,                            k41[68]);
buf     U6068c(    mc[69] ,                                       k41[68]);
not     U6069s(               ms[69] ,                            k41[69]);
buf     U6069c(    mc[70] ,                                       k41[69]);
not     U6070s(               ms[70] ,                            k41[70]);
buf     U6070c(    mc[71] ,                                       k41[70]);
not     U6071s(               ms[71] ,                            k41[71]);
buf     U6071c(    mc[72] ,                                       k41[71]);
not     U6072s(               ms[72] ,                            k41[72]);
buf     U6072c(    mc[73] ,                                       k41[72]);
not     U6073s(               ms[73] ,                            k41[73]);
buf     U6073c(    mc[74] ,                                       k41[73]);
not     U6074s(               ms[74] ,                            k41[74]);
buf     U6074c(    mc[75] ,                                       k41[74]);
endmodule
module fpmdsh (opc, mc, dsc, dshc);
output [74:1] opc;
input  [23:0] mc;
input   [6:0] dsc;
input   [1:0] dshc;
wire [23:0] mi;
wire        i0;
wire        i1;
wire [24:0] s0;
wire [26:0] s1;
wire [30:0] s2;
wire [38:0] s3;
wire [54:0] s4;
wire [74:1] s5;
assign
   mi[23:0] = {24{dshc[0]}} &  mc[23:0]
            | {24{dshc[1]}} & ~mc[23:0];
assign
   i0 =  dshc[1],
   i1 = ~dshc[1],
   s0[24:0]  = ~(dsc[0] ? {    i0  ,mi[23:0]}  : {mi[23:0],    i0  }),
   s1[26:0]  = ~(dsc[1] ? { {2{i1}},s0[24:0]}  : {s0[24:0], {2{i1}}}),
   s2[30:0]  = ~(dsc[2] ? { {4{i0}},s1[26:0]}  : {s1[26:0], {4{i0}}}),
   s3[38:0]  = ~(dsc[3] ? { {8{i1}},s2[30:0]}  : {s2[30:0], {8{i1}}}),
   s4[54:0]  = ~(dsc[4] ? {{16{i0}},s3[38:0]}  : {s3[38:0],{16{i0}}}),
   s5[74:1]  = ~(dsc[5] ? {{32{i1}},s4[54:13]} : {s4[54:0],{19{i1}}}),
   opc[74:1] =  (dsc[6] ? {{64{i0}},s5[74:65]} :  s5[74:1]);
endmodule
module fpmdsk (stky, mc, dsc, dshc);
output        stky;
input  [23:0] mc;
input   [6:0] dsc;
input   [1:0] dshc;
reg           sticky;
always @(dsc or mc)
   casez (dsc[6:0])
   7'b00?????: sticky =      1'b0;
   7'b010????: sticky =      1'b0;
   7'b0110000: sticky =      1'b0;
   7'b0110001: sticky =      1'b0;
   7'b0110010: sticky =      1'b0;
   7'b0110011: sticky =     mc[0];
   7'b0110100: sticky = |mc[01:0];
   7'b0110101: sticky = |mc[02:0];
   7'b0110110: sticky = |mc[03:0];
   7'b0110111: sticky = |mc[04:0];
   7'b0111000: sticky = |mc[05:0];
   7'b0111001: sticky = |mc[06:0];
   7'b0111010: sticky = |mc[07:0];
   7'b0111011: sticky = |mc[08:0];
   7'b0111100: sticky = |mc[09:0];
   7'b0111101: sticky = |mc[10:0];
   7'b0111110: sticky = |mc[11:0];
   7'b0111111: sticky = |mc[12:0];
   7'b1000000: sticky = |mc[13:0];
   7'b1000001: sticky = |mc[14:0];
   7'b1000010: sticky = |mc[15:0];
   7'b1000011: sticky = |mc[16:0];
   7'b1000100: sticky = |mc[17:0];
   7'b1000101: sticky = |mc[18:0];
   7'b1000110: sticky = |mc[19:0];
   7'b1000111: sticky = |mc[20:0];
   7'b1001000: sticky = |mc[21:0];
   7'b1001001: sticky = |mc[22:0];
   7'b1001010: sticky = |mc[23:0];
   default:    sticky =  1'bx;
   endcase
assign
   stky = dshc[0] &  sticky
        | dshc[1] & ~sticky;
endmodule
module fpme2 (e2, dscunf, SrcA, SrcB, SrcC);
output   [9:0] e2;
input          dscunf;
input  [30:23] SrcA;
input  [30:23] SrcB;
input  [30:23] SrcC;
assign
   e2[9:0] = dscunf ? {2'b0,SrcC[30:23]} + 10'b1
                    : {2'b0,SrcA[30:23]} + {2'b0,SrcB[30:23]} + 10'h39d;
endmodule
module fpmadd (dmr, g, p, ms, mc, sub);
output [74:0] dmr;
output        g;
output        p;
input  [74:0] ms;
input  [75:1] mc;
input         sub;
wire [75:0] g0;
wire [75:0] g1;
wire [75:0] g2;
wire [75:0] g3;
wire [75:0] g4;
wire [75:0] g5;
wire [75:0] g6;
wire [73:0] g7;
wire        g7_75;
wire [75:0] p0;
wire [75:0] p1;
wire [75:0] p2;
wire [75:0] p3;
wire [75:0] p4;
wire [75:0] p5;
wire [75:0] p6;
wire [74:0] x;
wire        subn;
wire        inv;
wire        g0_CIN;
wire        p0_CIN;
wire        g1_CIN;
wire        p1_CIN;
wire        g2_CIN;
wire        p2_CIN;
wire        g3_CIN;
wire        p3_CIN;
wire        g4_CIN;
wire        p4_CIN;
wire        g5_CIN;
wire        p5_CIN;
wire        g6_CIN;
wire        p6_CIN;
wire        g7_CIN;
assign subn   = ~sub;
assign g0[00] = ~(ms[00] & 1'b0  );
assign g0[01] = ~(ms[01] & mc[01]);
assign g0[02] = ~(ms[02] & mc[02]);
assign g0[03] = ~(ms[03] & mc[03]);
assign g0[04] = ~(ms[04] & mc[04]);
assign g0[05] = ~(ms[05] & mc[05]);
assign g0[06] = ~(ms[06] & mc[06]);
assign g0[07] = ~(ms[07] & mc[07]);
assign g0[08] = ~(ms[08] & mc[08]);
assign g0[09] = ~(ms[09] & mc[09]);
assign g0[10] = ~(ms[10] & mc[10]);
assign g0[11] = ~(ms[11] & mc[11]);
assign g0[12] = ~(ms[12] & mc[12]);
assign g0[13] = ~(ms[13] & mc[13]);
assign g0[14] = ~(ms[14] & mc[14]);
assign g0[15] = ~(ms[15] & mc[15]);
assign g0[16] = ~(ms[16] & mc[16]);
assign g0[17] = ~(ms[17] & mc[17]);
assign g0[18] = ~(ms[18] & mc[18]);
assign g0[19] = ~(ms[19] & mc[19]);
assign g0[20] = ~(ms[20] & mc[20]);
assign g0[21] = ~(ms[21] & mc[21]);
assign g0[22] = ~(ms[22] & mc[22]);
assign g0[23] = ~(ms[23] & mc[23]);
assign g0[24] = ~(ms[24] & mc[24]);
assign g0[25] = ~(ms[25] & mc[25]);
assign g0[26] = ~(ms[26] & mc[26]);
assign g0[27] = ~(ms[27] & mc[27]);
assign g0[28] = ~(ms[28] & mc[28]);
assign g0[29] = ~(ms[29] & mc[29]);
assign g0[30] = ~(ms[30] & mc[30]);
assign g0[31] = ~(ms[31] & mc[31]);
assign g0[32] = ~(ms[32] & mc[32]);
assign g0[33] = ~(ms[33] & mc[33]);
assign g0[34] = ~(ms[34] & mc[34]);
assign g0[35] = ~(ms[35] & mc[35]);
assign g0[36] = ~(ms[36] & mc[36]);
assign g0[37] = ~(ms[37] & mc[37]);
assign g0[38] = ~(ms[38] & mc[38]);
assign g0[39] = ~(ms[39] & mc[39]);
assign g0[40] = ~(ms[40] & mc[40]);
assign g0[41] = ~(ms[41] & mc[41]);
assign g0[42] = ~(ms[42] & mc[42]);
assign g0[43] = ~(ms[43] & mc[43]);
assign g0[44] = ~(ms[44] & mc[44]);
assign g0[45] = ~(ms[45] & mc[45]);
assign g0[46] = ~(ms[46] & mc[46]);
assign g0[47] = ~(ms[47] & mc[47]);
assign g0[48] = ~(ms[48] & mc[48]);
assign g0[49] = ~(ms[49] & mc[49]);
assign g0[50] = ~(ms[50] & mc[50]);
assign g0[51] = ~(ms[51] & mc[51]);
assign g0[52] = ~(ms[52] & mc[52]);
assign g0[53] = ~(ms[53] & mc[53]);
assign g0[54] = ~(ms[54] & mc[54]);
assign g0[55] = ~(ms[55] & mc[55]);
assign g0[56] = ~(ms[56] & mc[56]);
assign g0[57] = ~(ms[57] & mc[57]);
assign g0[58] = ~(ms[58] & mc[58]);
assign g0[59] = ~(ms[59] & mc[59]);
assign g0[60] = ~(ms[60] & mc[60]);
assign g0[61] = ~(ms[61] & mc[61]);
assign g0[62] = ~(ms[62] & mc[62]);
assign g0[63] = ~(ms[63] & mc[63]);
assign g0[64] = ~(ms[64] & mc[64]);
assign g0[65] = ~(ms[65] & mc[65]);
assign g0[66] = ~(ms[66] & mc[66]);
assign g0[67] = ~(ms[67] & mc[67]);
assign g0[68] = ~(ms[68] & mc[68]);
assign g0[69] = ~(ms[69] & mc[69]);
assign g0[70] = ~(ms[70] & mc[70]);
assign g0[71] = ~(ms[71] & mc[71]);
assign g0[72] = ~(ms[72] & mc[72]);
assign g0[73] = ~(ms[73] & mc[73]);
assign g0[74] = ~(ms[74] & mc[74]);
assign g0[75] = ~(  subn & mc[75]);
assign g0_CIN = 1'b1;
assign p0[00] = ~(ms[00] |  1'b0 );
assign p0[01] = ~(ms[01] | mc[01]);
assign p0[02] = ~(ms[02] | mc[02]);
assign p0[03] = ~(ms[03] | mc[03]);
assign p0[04] = ~(ms[04] | mc[04]);
assign p0[05] = ~(ms[05] | mc[05]);
assign p0[06] = ~(ms[06] | mc[06]);
assign p0[07] = ~(ms[07] | mc[07]);
assign p0[08] = ~(ms[08] | mc[08]);
assign p0[09] = ~(ms[09] | mc[09]);
assign p0[10] = ~(ms[10] | mc[10]);
assign p0[11] = ~(ms[11] | mc[11]);
assign p0[12] = ~(ms[12] | mc[12]);
assign p0[13] = ~(ms[13] | mc[13]);
assign p0[14] = ~(ms[14] | mc[14]);
assign p0[15] = ~(ms[15] | mc[15]);
assign p0[16] = ~(ms[16] | mc[16]);
assign p0[17] = ~(ms[17] | mc[17]);
assign p0[18] = ~(ms[18] | mc[18]);
assign p0[19] = ~(ms[19] | mc[19]);
assign p0[20] = ~(ms[20] | mc[20]);
assign p0[21] = ~(ms[21] | mc[21]);
assign p0[22] = ~(ms[22] | mc[22]);
assign p0[23] = ~(ms[23] | mc[23]);
assign p0[24] = ~(ms[24] | mc[24]);
assign p0[25] = ~(ms[25] | mc[25]);
assign p0[26] = ~(ms[26] | mc[26]);
assign p0[27] = ~(ms[27] | mc[27]);
assign p0[28] = ~(ms[28] | mc[28]);
assign p0[29] = ~(ms[29] | mc[29]);
assign p0[30] = ~(ms[30] | mc[30]);
assign p0[31] = ~(ms[31] | mc[31]);
assign p0[32] = ~(ms[32] | mc[32]);
assign p0[33] = ~(ms[33] | mc[33]);
assign p0[34] = ~(ms[34] | mc[34]);
assign p0[35] = ~(ms[35] | mc[35]);
assign p0[36] = ~(ms[36] | mc[36]);
assign p0[37] = ~(ms[37] | mc[37]);
assign p0[38] = ~(ms[38] | mc[38]);
assign p0[39] = ~(ms[39] | mc[39]);
assign p0[40] = ~(ms[40] | mc[40]);
assign p0[41] = ~(ms[41] | mc[41]);
assign p0[42] = ~(ms[42] | mc[42]);
assign p0[43] = ~(ms[43] | mc[43]);
assign p0[44] = ~(ms[44] | mc[44]);
assign p0[45] = ~(ms[45] | mc[45]);
assign p0[46] = ~(ms[46] | mc[46]);
assign p0[47] = ~(ms[47] | mc[47]);
assign p0[48] = ~(ms[48] | mc[48]);
assign p0[49] = ~(ms[49] | mc[49]);
assign p0[50] = ~(ms[50] | mc[50]);
assign p0[51] = ~(ms[51] | mc[51]);
assign p0[52] = ~(ms[52] | mc[52]);
assign p0[53] = ~(ms[53] | mc[53]);
assign p0[54] = ~(ms[54] | mc[54]);
assign p0[55] = ~(ms[55] | mc[55]);
assign p0[56] = ~(ms[56] | mc[56]);
assign p0[57] = ~(ms[57] | mc[57]);
assign p0[58] = ~(ms[58] | mc[58]);
assign p0[59] = ~(ms[59] | mc[59]);
assign p0[60] = ~(ms[60] | mc[60]);
assign p0[61] = ~(ms[61] | mc[61]);
assign p0[62] = ~(ms[62] | mc[62]);
assign p0[63] = ~(ms[63] | mc[63]);
assign p0[64] = ~(ms[64] | mc[64]);
assign p0[65] = ~(ms[65] | mc[65]);
assign p0[66] = ~(ms[66] | mc[66]);
assign p0[67] = ~(ms[67] | mc[67]);
assign p0[68] = ~(ms[68] | mc[68]);
assign p0[69] = ~(ms[69] | mc[69]);
assign p0[70] = ~(ms[70] | mc[70]);
assign p0[71] = ~(ms[71] | mc[71]);
assign p0[72] = ~(ms[72] | mc[72]);
assign p0[73] = ~(ms[73] | mc[73]);
assign p0[74] = ~(ms[74] | mc[74]);
assign p0[75] = ~(  subn | mc[75]);
assign p0_CIN = ~(  sub  & mc[75]);
assign x[00] = (~g0[00] | p0[00]);
assign x[01] = (~g0[01] | p0[01]);
assign x[02] = (~g0[02] | p0[02]);
assign x[03] = (~g0[03] | p0[03]);
assign x[04] = (~g0[04] | p0[04]);
assign x[05] = (~g0[05] | p0[05]);
assign x[06] = (~g0[06] | p0[06]);
assign x[07] = (~g0[07] | p0[07]);
assign x[08] = (~g0[08] | p0[08]);
assign x[09] = (~g0[09] | p0[09]);
assign x[10] = (~g0[10] | p0[10]);
assign x[11] = (~g0[11] | p0[11]);
assign x[12] = (~g0[12] | p0[12]);
assign x[13] = (~g0[13] | p0[13]);
assign x[14] = (~g0[14] | p0[14]);
assign x[15] = (~g0[15] | p0[15]);
assign x[16] = (~g0[16] | p0[16]);
assign x[17] = (~g0[17] | p0[17]);
assign x[18] = (~g0[18] | p0[18]);
assign x[19] = (~g0[19] | p0[19]);
assign x[20] = (~g0[20] | p0[20]);
assign x[21] = (~g0[21] | p0[21]);
assign x[22] = (~g0[22] | p0[22]);
assign x[23] = (~g0[23] | p0[23]);
assign x[24] = (~g0[24] | p0[24]);
assign x[25] = (~g0[25] | p0[25]);
assign x[26] = (~g0[26] | p0[26]);
assign x[27] = (~g0[27] | p0[27]);
assign x[28] = (~g0[28] | p0[28]);
assign x[29] = (~g0[29] | p0[29]);
assign x[30] = (~g0[30] | p0[30]);
assign x[31] = (~g0[31] | p0[31]);
assign x[32] = (~g0[32] | p0[32]);
assign x[33] = (~g0[33] | p0[33]);
assign x[34] = (~g0[34] | p0[34]);
assign x[35] = (~g0[35] | p0[35]);
assign x[36] = (~g0[36] | p0[36]);
assign x[37] = (~g0[37] | p0[37]);
assign x[38] = (~g0[38] | p0[38]);
assign x[39] = (~g0[39] | p0[39]);
assign x[40] = (~g0[40] | p0[40]);
assign x[41] = (~g0[41] | p0[41]);
assign x[42] = (~g0[42] | p0[42]);
assign x[43] = (~g0[43] | p0[43]);
assign x[44] = (~g0[44] | p0[44]);
assign x[45] = (~g0[45] | p0[45]);
assign x[46] = (~g0[46] | p0[46]);
assign x[47] = (~g0[47] | p0[47]);
assign x[48] = (~g0[48] | p0[48]);
assign x[49] = (~g0[49] | p0[49]);
assign x[50] = (~g0[50] | p0[50]);
assign x[51] = (~g0[51] | p0[51]);
assign x[52] = (~g0[52] | p0[52]);
assign x[53] = (~g0[53] | p0[53]);
assign x[54] = (~g0[54] | p0[54]);
assign x[55] = (~g0[55] | p0[55]);
assign x[56] = (~g0[56] | p0[56]);
assign x[57] = (~g0[57] | p0[57]);
assign x[58] = (~g0[58] | p0[58]);
assign x[59] = (~g0[59] | p0[59]);
assign x[60] = (~g0[60] | p0[60]);
assign x[61] = (~g0[61] | p0[61]);
assign x[62] = (~g0[62] | p0[62]);
assign x[63] = (~g0[63] | p0[63]);
assign x[64] = (~g0[64] | p0[64]);
assign x[65] = (~g0[65] | p0[65]);
assign x[66] = (~g0[66] | p0[66]);
assign x[67] = (~g0[67] | p0[67]);
assign x[68] = (~g0[68] | p0[68]);
assign x[69] = (~g0[69] | p0[69]);
assign x[70] = (~g0[70] | p0[70]);
assign x[71] = (~g0[71] | p0[71]);
assign x[72] = (~g0[72] | p0[72]);
assign x[73] = (~g0[73] | p0[73]);
assign x[74] = (~g0[74] | p0[74]);
assign g1[00] = ~(g0[00] & (p0[00] | g0_CIN));
assign g1[01] = ~(g0[01] & (p0[01] | g0[00]));
assign g1[02] = ~(g0[02] & (p0[02] | g0[01]));
assign g1[03] = ~(g0[03] & (p0[03] | g0[02]));
assign g1[04] = ~(g0[04] & (p0[04] | g0[03]));
assign g1[05] = ~(g0[05] & (p0[05] | g0[04]));
assign g1[06] = ~(g0[06] & (p0[06] | g0[05]));
assign g1[07] = ~(g0[07] & (p0[07] | g0[06]));
assign g1[08] = ~(g0[08] & (p0[08] | g0[07]));
assign g1[09] = ~(g0[09] & (p0[09] | g0[08]));
assign g1[10] = ~(g0[10] & (p0[10] | g0[09]));
assign g1[11] = ~(g0[11] & (p0[11] | g0[10]));
assign g1[12] = ~(g0[12] & (p0[12] | g0[11]));
assign g1[13] = ~(g0[13] & (p0[13] | g0[12]));
assign g1[14] = ~(g0[14] & (p0[14] | g0[13]));
assign g1[15] = ~(g0[15] & (p0[15] | g0[14]));
assign g1[16] = ~(g0[16] & (p0[16] | g0[15]));
assign g1[17] = ~(g0[17] & (p0[17] | g0[16]));
assign g1[18] = ~(g0[18] & (p0[18] | g0[17]));
assign g1[19] = ~(g0[19] & (p0[19] | g0[18]));
assign g1[20] = ~(g0[20] & (p0[20] | g0[19]));
assign g1[21] = ~(g0[21] & (p0[21] | g0[20]));
assign g1[22] = ~(g0[22] & (p0[22] | g0[21]));
assign g1[23] = ~(g0[23] & (p0[23] | g0[22]));
assign g1[24] = ~(g0[24] & (p0[24] | g0[23]));
assign g1[25] = ~(g0[25] & (p0[25] | g0[24]));
assign g1[26] = ~(g0[26] & (p0[26] | g0[25]));
assign g1[27] = ~(g0[27] & (p0[27] | g0[26]));
assign g1[28] = ~(g0[28] & (p0[28] | g0[27]));
assign g1[29] = ~(g0[29] & (p0[29] | g0[28]));
assign g1[30] = ~(g0[30] & (p0[30] | g0[29]));
assign g1[31] = ~(g0[31] & (p0[31] | g0[30]));
assign g1[32] = ~(g0[32] & (p0[32] | g0[31]));
assign g1[33] = ~(g0[33] & (p0[33] | g0[32]));
assign g1[34] = ~(g0[34] & (p0[34] | g0[33]));
assign g1[35] = ~(g0[35] & (p0[35] | g0[34]));
assign g1[36] = ~(g0[36] & (p0[36] | g0[35]));
assign g1[37] = ~(g0[37] & (p0[37] | g0[36]));
assign g1[38] = ~(g0[38] & (p0[38] | g0[37]));
assign g1[39] = ~(g0[39] & (p0[39] | g0[38]));
assign g1[40] = ~(g0[40] & (p0[40] | g0[39]));
assign g1[41] = ~(g0[41] & (p0[41] | g0[40]));
assign g1[42] = ~(g0[42] & (p0[42] | g0[41]));
assign g1[43] = ~(g0[43] & (p0[43] | g0[42]));
assign g1[44] = ~(g0[44] & (p0[44] | g0[43]));
assign g1[45] = ~(g0[45] & (p0[45] | g0[44]));
assign g1[46] = ~(g0[46] & (p0[46] | g0[45]));
assign g1[47] = ~(g0[47] & (p0[47] | g0[46]));
assign g1[48] = ~(g0[48] & (p0[48] | g0[47]));
assign g1[49] = ~(g0[49] & (p0[49] | g0[48]));
assign g1[50] = ~(g0[50] & (p0[50] | g0[49]));
assign g1[51] = ~(g0[51] & (p0[51] | g0[50]));
assign g1[52] = ~(g0[52] & (p0[52] | g0[51]));
assign g1[53] = ~(g0[53] & (p0[53] | g0[52]));
assign g1[54] = ~(g0[54] & (p0[54] | g0[53]));
assign g1[55] = ~(g0[55] & (p0[55] | g0[54]));
assign g1[56] = ~(g0[56] & (p0[56] | g0[55]));
assign g1[57] = ~(g0[57] & (p0[57] | g0[56]));
assign g1[58] = ~(g0[58] & (p0[58] | g0[57]));
assign g1[59] = ~(g0[59] & (p0[59] | g0[58]));
assign g1[60] = ~(g0[60] & (p0[60] | g0[59]));
assign g1[61] = ~(g0[61] & (p0[61] | g0[60]));
assign g1[62] = ~(g0[62] & (p0[62] | g0[61]));
assign g1[63] = ~(g0[63] & (p0[63] | g0[62]));
assign g1[64] = ~(g0[64] & (p0[64] | g0[63]));
assign g1[65] = ~(g0[65] & (p0[65] | g0[64]));
assign g1[66] = ~(g0[66] & (p0[66] | g0[65]));
assign g1[67] = ~(g0[67] & (p0[67] | g0[66]));
assign g1[68] = ~(g0[68] & (p0[68] | g0[67]));
assign g1[69] = ~(g0[69] & (p0[69] | g0[68]));
assign g1[70] = ~(g0[70] & (p0[70] | g0[69]));
assign g1[71] = ~(g0[71] & (p0[71] | g0[70]));
assign g1[72] = ~(g0[72] & (p0[72] | g0[71]));
assign g1[73] = ~(g0[73] & (p0[73] | g0[72]));
assign g1[74] = ~(g0[74] & (p0[74] | g0[73]));
assign g1[75] = ~(g0[75] & (p0[75] | g0[74]));
assign g1_CIN = ~(g0_CIN & (p0_CIN | g0[74]));
assign p1[00] = ~(p0[00] | p0_CIN);
assign p1[01] = ~(p0[01] | p0[00]);
assign p1[02] = ~(p0[02] | p0[01]);
assign p1[03] = ~(p0[03] | p0[02]);
assign p1[04] = ~(p0[04] | p0[03]);
assign p1[05] = ~(p0[05] | p0[04]);
assign p1[06] = ~(p0[06] | p0[05]);
assign p1[07] = ~(p0[07] | p0[06]);
assign p1[08] = ~(p0[08] | p0[07]);
assign p1[09] = ~(p0[09] | p0[08]);
assign p1[10] = ~(p0[10] | p0[09]);
assign p1[11] = ~(p0[11] | p0[10]);
assign p1[12] = ~(p0[12] | p0[11]);
assign p1[13] = ~(p0[13] | p0[12]);
assign p1[14] = ~(p0[14] | p0[13]);
assign p1[15] = ~(p0[15] | p0[14]);
assign p1[16] = ~(p0[16] | p0[15]);
assign p1[17] = ~(p0[17] | p0[16]);
assign p1[18] = ~(p0[18] | p0[17]);
assign p1[19] = ~(p0[19] | p0[18]);
assign p1[20] = ~(p0[20] | p0[19]);
assign p1[21] = ~(p0[21] | p0[20]);
assign p1[22] = ~(p0[22] | p0[21]);
assign p1[23] = ~(p0[23] | p0[22]);
assign p1[24] = ~(p0[24] | p0[23]);
assign p1[25] = ~(p0[25] | p0[24]);
assign p1[26] = ~(p0[26] | p0[25]);
assign p1[27] = ~(p0[27] | p0[26]);
assign p1[28] = ~(p0[28] | p0[27]);
assign p1[29] = ~(p0[29] | p0[28]);
assign p1[30] = ~(p0[30] | p0[29]);
assign p1[31] = ~(p0[31] | p0[30]);
assign p1[32] = ~(p0[32] | p0[31]);
assign p1[33] = ~(p0[33] | p0[32]);
assign p1[34] = ~(p0[34] | p0[33]);
assign p1[35] = ~(p0[35] | p0[34]);
assign p1[36] = ~(p0[36] | p0[35]);
assign p1[37] = ~(p0[37] | p0[36]);
assign p1[38] = ~(p0[38] | p0[37]);
assign p1[39] = ~(p0[39] | p0[38]);
assign p1[40] = ~(p0[40] | p0[39]);
assign p1[41] = ~(p0[41] | p0[40]);
assign p1[42] = ~(p0[42] | p0[41]);
assign p1[43] = ~(p0[43] | p0[42]);
assign p1[44] = ~(p0[44] | p0[43]);
assign p1[45] = ~(p0[45] | p0[44]);
assign p1[46] = ~(p0[46] | p0[45]);
assign p1[47] = ~(p0[47] | p0[46]);
assign p1[48] = ~(p0[48] | p0[47]);
assign p1[49] = ~(p0[49] | p0[48]);
assign p1[50] = ~(p0[50] | p0[49]);
assign p1[51] = ~(p0[51] | p0[50]);
assign p1[52] = ~(p0[52] | p0[51]);
assign p1[53] = ~(p0[53] | p0[52]);
assign p1[54] = ~(p0[54] | p0[53]);
assign p1[55] = ~(p0[55] | p0[54]);
assign p1[56] = ~(p0[56] | p0[55]);
assign p1[57] = ~(p0[57] | p0[56]);
assign p1[58] = ~(p0[58] | p0[57]);
assign p1[59] = ~(p0[59] | p0[58]);
assign p1[60] = ~(p0[60] | p0[59]);
assign p1[61] = ~(p0[61] | p0[60]);
assign p1[62] = ~(p0[62] | p0[61]);
assign p1[63] = ~(p0[63] | p0[62]);
assign p1[64] = ~(p0[64] | p0[63]);
assign p1[65] = ~(p0[65] | p0[64]);
assign p1[66] = ~(p0[66] | p0[65]);
assign p1[67] = ~(p0[67] | p0[66]);
assign p1[68] = ~(p0[68] | p0[67]);
assign p1[69] = ~(p0[69] | p0[68]);
assign p1[70] = ~(p0[70] | p0[69]);
assign p1[71] = ~(p0[71] | p0[70]);
assign p1[72] = ~(p0[72] | p0[71]);
assign p1[73] = ~(p0[73] | p0[72]);
assign p1[74] = ~(p0[74] | p0[73]);
assign p1[75] = ~(p0[75] | p0[74]);
assign p1_CIN = ~(p0_CIN | p0[74]);
assign g2[00] = ~(g1[00] | (p1[00] & g1[74]));
assign g2[01] = ~(g1[01] | (p1[01] & g1_CIN));
assign g2[02] = ~(g1[02] | (p1[02] & g1[00]));
assign g2[03] = ~(g1[03] | (p1[03] & g1[01]));
assign g2[04] = ~(g1[04] | (p1[04] & g1[02]));
assign g2[05] = ~(g1[05] | (p1[05] & g1[03]));
assign g2[06] = ~(g1[06] | (p1[06] & g1[04]));
assign g2[07] = ~(g1[07] | (p1[07] & g1[05]));
assign g2[08] = ~(g1[08] | (p1[08] & g1[06]));
assign g2[09] = ~(g1[09] | (p1[09] & g1[07]));
assign g2[10] = ~(g1[10] | (p1[10] & g1[08]));
assign g2[11] = ~(g1[11] | (p1[11] & g1[09]));
assign g2[12] = ~(g1[12] | (p1[12] & g1[10]));
assign g2[13] = ~(g1[13] | (p1[13] & g1[11]));
assign g2[14] = ~(g1[14] | (p1[14] & g1[12]));
assign g2[15] = ~(g1[15] | (p1[15] & g1[13]));
assign g2[16] = ~(g1[16] | (p1[16] & g1[14]));
assign g2[17] = ~(g1[17] | (p1[17] & g1[15]));
assign g2[18] = ~(g1[18] | (p1[18] & g1[16]));
assign g2[19] = ~(g1[19] | (p1[19] & g1[17]));
assign g2[20] = ~(g1[20] | (p1[20] & g1[18]));
assign g2[21] = ~(g1[21] | (p1[21] & g1[19]));
assign g2[22] = ~(g1[22] | (p1[22] & g1[20]));
assign g2[23] = ~(g1[23] | (p1[23] & g1[21]));
assign g2[24] = ~(g1[24] | (p1[24] & g1[22]));
assign g2[25] = ~(g1[25] | (p1[25] & g1[23]));
assign g2[26] = ~(g1[26] | (p1[26] & g1[24]));
assign g2[27] = ~(g1[27] | (p1[27] & g1[25]));
assign g2[28] = ~(g1[28] | (p1[28] & g1[26]));
assign g2[29] = ~(g1[29] | (p1[29] & g1[27]));
assign g2[30] = ~(g1[30] | (p1[30] & g1[28]));
assign g2[31] = ~(g1[31] | (p1[31] & g1[29]));
assign g2[32] = ~(g1[32] | (p1[32] & g1[30]));
assign g2[33] = ~(g1[33] | (p1[33] & g1[31]));
assign g2[34] = ~(g1[34] | (p1[34] & g1[32]));
assign g2[35] = ~(g1[35] | (p1[35] & g1[33]));
assign g2[36] = ~(g1[36] | (p1[36] & g1[34]));
assign g2[37] = ~(g1[37] | (p1[37] & g1[35]));
assign g2[38] = ~(g1[38] | (p1[38] & g1[36]));
assign g2[39] = ~(g1[39] | (p1[39] & g1[37]));
assign g2[40] = ~(g1[40] | (p1[40] & g1[38]));
assign g2[41] = ~(g1[41] | (p1[41] & g1[39]));
assign g2[42] = ~(g1[42] | (p1[42] & g1[40]));
assign g2[43] = ~(g1[43] | (p1[43] & g1[41]));
assign g2[44] = ~(g1[44] | (p1[44] & g1[42]));
assign g2[45] = ~(g1[45] | (p1[45] & g1[43]));
assign g2[46] = ~(g1[46] | (p1[46] & g1[44]));
assign g2[47] = ~(g1[47] | (p1[47] & g1[45]));
assign g2[48] = ~(g1[48] | (p1[48] & g1[46]));
assign g2[49] = ~(g1[49] | (p1[49] & g1[47]));
assign g2[50] = ~(g1[50] | (p1[50] & g1[48]));
assign g2[51] = ~(g1[51] | (p1[51] & g1[49]));
assign g2[52] = ~(g1[52] | (p1[52] & g1[50]));
assign g2[53] = ~(g1[53] | (p1[53] & g1[51]));
assign g2[54] = ~(g1[54] | (p1[54] & g1[52]));
assign g2[55] = ~(g1[55] | (p1[55] & g1[53]));
assign g2[56] = ~(g1[56] | (p1[56] & g1[54]));
assign g2[57] = ~(g1[57] | (p1[57] & g1[55]));
assign g2[58] = ~(g1[58] | (p1[58] & g1[56]));
assign g2[59] = ~(g1[59] | (p1[59] & g1[57]));
assign g2[60] = ~(g1[60] | (p1[60] & g1[58]));
assign g2[61] = ~(g1[61] | (p1[61] & g1[59]));
assign g2[62] = ~(g1[62] | (p1[62] & g1[60]));
assign g2[63] = ~(g1[63] | (p1[63] & g1[61]));
assign g2[64] = ~(g1[64] | (p1[64] & g1[62]));
assign g2[65] = ~(g1[65] | (p1[65] & g1[63]));
assign g2[66] = ~(g1[66] | (p1[66] & g1[64]));
assign g2[67] = ~(g1[67] | (p1[67] & g1[65]));
assign g2[68] = ~(g1[68] | (p1[68] & g1[66]));
assign g2[69] = ~(g1[69] | (p1[69] & g1[67]));
assign g2[70] = ~(g1[70] | (p1[70] & g1[68]));
assign g2[71] = ~(g1[71] | (p1[71] & g1[69]));
assign g2[72] = ~(g1[72] | (p1[72] & g1[70]));
assign g2[73] = ~(g1[73] | (p1[73] & g1[71]));
assign g2[74] = ~(g1[74] | (p1[74] & g1[72]));
assign g2[75] = ~(g1[75] | (p1[75] & g1[73]));
assign g2_CIN = ~(g1_CIN | (p1_CIN & g1[73]));
assign p2[00] = ~(p1[00] & p1[74]);
assign p2[01] = ~(p1[01] & p1_CIN);
assign p2[02] = ~(p1[02] & p1[00]);
assign p2[03] = ~(p1[03] & p1[01]);
assign p2[04] = ~(p1[04] & p1[02]);
assign p2[05] = ~(p1[05] & p1[03]);
assign p2[06] = ~(p1[06] & p1[04]);
assign p2[07] = ~(p1[07] & p1[05]);
assign p2[08] = ~(p1[08] & p1[06]);
assign p2[09] = ~(p1[09] & p1[07]);
assign p2[10] = ~(p1[10] & p1[08]);
assign p2[11] = ~(p1[11] & p1[09]);
assign p2[12] = ~(p1[12] & p1[10]);
assign p2[13] = ~(p1[13] & p1[11]);
assign p2[14] = ~(p1[14] & p1[12]);
assign p2[15] = ~(p1[15] & p1[13]);
assign p2[16] = ~(p1[16] & p1[14]);
assign p2[17] = ~(p1[17] & p1[15]);
assign p2[18] = ~(p1[18] & p1[16]);
assign p2[19] = ~(p1[19] & p1[17]);
assign p2[20] = ~(p1[20] & p1[18]);
assign p2[21] = ~(p1[21] & p1[19]);
assign p2[22] = ~(p1[22] & p1[20]);
assign p2[23] = ~(p1[23] & p1[21]);
assign p2[24] = ~(p1[24] & p1[22]);
assign p2[25] = ~(p1[25] & p1[23]);
assign p2[26] = ~(p1[26] & p1[24]);
assign p2[27] = ~(p1[27] & p1[25]);
assign p2[28] = ~(p1[28] & p1[26]);
assign p2[29] = ~(p1[29] & p1[27]);
assign p2[30] = ~(p1[30] & p1[28]);
assign p2[31] = ~(p1[31] & p1[29]);
assign p2[32] = ~(p1[32] & p1[30]);
assign p2[33] = ~(p1[33] & p1[31]);
assign p2[34] = ~(p1[34] & p1[32]);
assign p2[35] = ~(p1[35] & p1[33]);
assign p2[36] = ~(p1[36] & p1[34]);
assign p2[37] = ~(p1[37] & p1[35]);
assign p2[38] = ~(p1[38] & p1[36]);
assign p2[39] = ~(p1[39] & p1[37]);
assign p2[40] = ~(p1[40] & p1[38]);
assign p2[41] = ~(p1[41] & p1[39]);
assign p2[42] = ~(p1[42] & p1[40]);
assign p2[43] = ~(p1[43] & p1[41]);
assign p2[44] = ~(p1[44] & p1[42]);
assign p2[45] = ~(p1[45] & p1[43]);
assign p2[46] = ~(p1[46] & p1[44]);
assign p2[47] = ~(p1[47] & p1[45]);
assign p2[48] = ~(p1[48] & p1[46]);
assign p2[49] = ~(p1[49] & p1[47]);
assign p2[50] = ~(p1[50] & p1[48]);
assign p2[51] = ~(p1[51] & p1[49]);
assign p2[52] = ~(p1[52] & p1[50]);
assign p2[53] = ~(p1[53] & p1[51]);
assign p2[54] = ~(p1[54] & p1[52]);
assign p2[55] = ~(p1[55] & p1[53]);
assign p2[56] = ~(p1[56] & p1[54]);
assign p2[57] = ~(p1[57] & p1[55]);
assign p2[58] = ~(p1[58] & p1[56]);
assign p2[59] = ~(p1[59] & p1[57]);
assign p2[60] = ~(p1[60] & p1[58]);
assign p2[61] = ~(p1[61] & p1[59]);
assign p2[62] = ~(p1[62] & p1[60]);
assign p2[63] = ~(p1[63] & p1[61]);
assign p2[64] = ~(p1[64] & p1[62]);
assign p2[65] = ~(p1[65] & p1[63]);
assign p2[66] = ~(p1[66] & p1[64]);
assign p2[67] = ~(p1[67] & p1[65]);
assign p2[68] = ~(p1[68] & p1[66]);
assign p2[69] = ~(p1[69] & p1[67]);
assign p2[70] = ~(p1[70] & p1[68]);
assign p2[71] = ~(p1[71] & p1[69]);
assign p2[72] = ~(p1[72] & p1[70]);
assign p2[73] = ~(p1[73] & p1[71]);
assign p2[74] = ~(p1[74] & p1[72]);
assign p2[75] = ~(p1[75] & p1[73]);
assign p2_CIN = ~(p1_CIN & p1[73]);
assign g3[00] = ~(g2[00] & (p2[00] | g2[72]));
assign g3[01] = ~(g2[01] & (p2[01] | g2[73]));
assign g3[02] = ~(g2[02] & (p2[02] | g2[74]));
assign g3[03] = ~(g2[03] & (p2[03] | g2_CIN));
assign g3[04] = ~(g2[04] & (p2[04] | g2[00]));
assign g3[05] = ~(g2[05] & (p2[05] | g2[01]));
assign g3[06] = ~(g2[06] & (p2[06] | g2[02]));
assign g3[07] = ~(g2[07] & (p2[07] | g2[03]));
assign g3[08] = ~(g2[08] & (p2[08] | g2[04]));
assign g3[09] = ~(g2[09] & (p2[09] | g2[05]));
assign g3[10] = ~(g2[10] & (p2[10] | g2[06]));
assign g3[11] = ~(g2[11] & (p2[11] | g2[07]));
assign g3[12] = ~(g2[12] & (p2[12] | g2[08]));
assign g3[13] = ~(g2[13] & (p2[13] | g2[09]));
assign g3[14] = ~(g2[14] & (p2[14] | g2[10]));
assign g3[15] = ~(g2[15] & (p2[15] | g2[11]));
assign g3[16] = ~(g2[16] & (p2[16] | g2[12]));
assign g3[17] = ~(g2[17] & (p2[17] | g2[13]));
assign g3[18] = ~(g2[18] & (p2[18] | g2[14]));
assign g3[19] = ~(g2[19] & (p2[19] | g2[15]));
assign g3[20] = ~(g2[20] & (p2[20] | g2[16]));
assign g3[21] = ~(g2[21] & (p2[21] | g2[17]));
assign g3[22] = ~(g2[22] & (p2[22] | g2[18]));
assign g3[23] = ~(g2[23] & (p2[23] | g2[19]));
assign g3[24] = ~(g2[24] & (p2[24] | g2[20]));
assign g3[25] = ~(g2[25] & (p2[25] | g2[21]));
assign g3[26] = ~(g2[26] & (p2[26] | g2[22]));
assign g3[27] = ~(g2[27] & (p2[27] | g2[23]));
assign g3[28] = ~(g2[28] & (p2[28] | g2[24]));
assign g3[29] = ~(g2[29] & (p2[29] | g2[25]));
assign g3[30] = ~(g2[30] & (p2[30] | g2[26]));
assign g3[31] = ~(g2[31] & (p2[31] | g2[27]));
assign g3[32] = ~(g2[32] & (p2[32] | g2[28]));
assign g3[33] = ~(g2[33] & (p2[33] | g2[29]));
assign g3[34] = ~(g2[34] & (p2[34] | g2[30]));
assign g3[35] = ~(g2[35] & (p2[35] | g2[31]));
assign g3[36] = ~(g2[36] & (p2[36] | g2[32]));
assign g3[37] = ~(g2[37] & (p2[37] | g2[33]));
assign g3[38] = ~(g2[38] & (p2[38] | g2[34]));
assign g3[39] = ~(g2[39] & (p2[39] | g2[35]));
assign g3[40] = ~(g2[40] & (p2[40] | g2[36]));
assign g3[41] = ~(g2[41] & (p2[41] | g2[37]));
assign g3[42] = ~(g2[42] & (p2[42] | g2[38]));
assign g3[43] = ~(g2[43] & (p2[43] | g2[39]));
assign g3[44] = ~(g2[44] & (p2[44] | g2[40]));
assign g3[45] = ~(g2[45] & (p2[45] | g2[41]));
assign g3[46] = ~(g2[46] & (p2[46] | g2[42]));
assign g3[47] = ~(g2[47] & (p2[47] | g2[43]));
assign g3[48] = ~(g2[48] & (p2[48] | g2[44]));
assign g3[49] = ~(g2[49] & (p2[49] | g2[45]));
assign g3[50] = ~(g2[50] & (p2[50] | g2[46]));
assign g3[51] = ~(g2[51] & (p2[51] | g2[47]));
assign g3[52] = ~(g2[52] & (p2[52] | g2[48]));
assign g3[53] = ~(g2[53] & (p2[53] | g2[49]));
assign g3[54] = ~(g2[54] & (p2[54] | g2[50]));
assign g3[55] = ~(g2[55] & (p2[55] | g2[51]));
assign g3[56] = ~(g2[56] & (p2[56] | g2[52]));
assign g3[57] = ~(g2[57] & (p2[57] | g2[53]));
assign g3[58] = ~(g2[58] & (p2[58] | g2[54]));
assign g3[59] = ~(g2[59] & (p2[59] | g2[55]));
assign g3[60] = ~(g2[60] & (p2[60] | g2[56]));
assign g3[61] = ~(g2[61] & (p2[61] | g2[57]));
assign g3[62] = ~(g2[62] & (p2[62] | g2[58]));
assign g3[63] = ~(g2[63] & (p2[63] | g2[59]));
assign g3[64] = ~(g2[64] & (p2[64] | g2[60]));
assign g3[65] = ~(g2[65] & (p2[65] | g2[61]));
assign g3[66] = ~(g2[66] & (p2[66] | g2[62]));
assign g3[67] = ~(g2[67] & (p2[67] | g2[63]));
assign g3[68] = ~(g2[68] & (p2[68] | g2[64]));
assign g3[69] = ~(g2[69] & (p2[69] | g2[65]));
assign g3[70] = ~(g2[70] & (p2[70] | g2[66]));
assign g3[71] = ~(g2[71] & (p2[71] | g2[67]));
assign g3[72] = ~(g2[72] & (p2[72] | g2[68]));
assign g3[73] = ~(g2[73] & (p2[73] | g2[69]));
assign g3[74] = ~(g2[74] & (p2[74] | g2[70]));
assign g3[75] = ~(g2[75] & (p2[75] | g2[71]));
assign g3_CIN = ~(g2_CIN & (p2_CIN | g2[71]));
assign p3[00] = ~(p2[00] | p2[72]);
assign p3[01] = ~(p2[01] | p2[73]);
assign p3[02] = ~(p2[02] | p2[74]);
assign p3[03] = ~(p2[03] | p2_CIN);
assign p3[04] = ~(p2[04] | p2[00]);
assign p3[05] = ~(p2[05] | p2[01]);
assign p3[06] = ~(p2[06] | p2[02]);
assign p3[07] = ~(p2[07] | p2[03]);
assign p3[08] = ~(p2[08] | p2[04]);
assign p3[09] = ~(p2[09] | p2[05]);
assign p3[10] = ~(p2[10] | p2[06]);
assign p3[11] = ~(p2[11] | p2[07]);
assign p3[12] = ~(p2[12] | p2[08]);
assign p3[13] = ~(p2[13] | p2[09]);
assign p3[14] = ~(p2[14] | p2[10]);
assign p3[15] = ~(p2[15] | p2[11]);
assign p3[16] = ~(p2[16] | p2[12]);
assign p3[17] = ~(p2[17] | p2[13]);
assign p3[18] = ~(p2[18] | p2[14]);
assign p3[19] = ~(p2[19] | p2[15]);
assign p3[20] = ~(p2[20] | p2[16]);
assign p3[21] = ~(p2[21] | p2[17]);
assign p3[22] = ~(p2[22] | p2[18]);
assign p3[23] = ~(p2[23] | p2[19]);
assign p3[24] = ~(p2[24] | p2[20]);
assign p3[25] = ~(p2[25] | p2[21]);
assign p3[26] = ~(p2[26] | p2[22]);
assign p3[27] = ~(p2[27] | p2[23]);
assign p3[28] = ~(p2[28] | p2[24]);
assign p3[29] = ~(p2[29] | p2[25]);
assign p3[30] = ~(p2[30] | p2[26]);
assign p3[31] = ~(p2[31] | p2[27]);
assign p3[32] = ~(p2[32] | p2[28]);
assign p3[33] = ~(p2[33] | p2[29]);
assign p3[34] = ~(p2[34] | p2[30]);
assign p3[35] = ~(p2[35] | p2[31]);
assign p3[36] = ~(p2[36] | p2[32]);
assign p3[37] = ~(p2[37] | p2[33]);
assign p3[38] = ~(p2[38] | p2[34]);
assign p3[39] = ~(p2[39] | p2[35]);
assign p3[40] = ~(p2[40] | p2[36]);
assign p3[41] = ~(p2[41] | p2[37]);
assign p3[42] = ~(p2[42] | p2[38]);
assign p3[43] = ~(p2[43] | p2[39]);
assign p3[44] = ~(p2[44] | p2[40]);
assign p3[45] = ~(p2[45] | p2[41]);
assign p3[46] = ~(p2[46] | p2[42]);
assign p3[47] = ~(p2[47] | p2[43]);
assign p3[48] = ~(p2[48] | p2[44]);
assign p3[49] = ~(p2[49] | p2[45]);
assign p3[50] = ~(p2[50] | p2[46]);
assign p3[51] = ~(p2[51] | p2[47]);
assign p3[52] = ~(p2[52] | p2[48]);
assign p3[53] = ~(p2[53] | p2[49]);
assign p3[54] = ~(p2[54] | p2[50]);
assign p3[55] = ~(p2[55] | p2[51]);
assign p3[56] = ~(p2[56] | p2[52]);
assign p3[57] = ~(p2[57] | p2[53]);
assign p3[58] = ~(p2[58] | p2[54]);
assign p3[59] = ~(p2[59] | p2[55]);
assign p3[60] = ~(p2[60] | p2[56]);
assign p3[61] = ~(p2[61] | p2[57]);
assign p3[62] = ~(p2[62] | p2[58]);
assign p3[63] = ~(p2[63] | p2[59]);
assign p3[64] = ~(p2[64] | p2[60]);
assign p3[65] = ~(p2[65] | p2[61]);
assign p3[66] = ~(p2[66] | p2[62]);
assign p3[67] = ~(p2[67] | p2[63]);
assign p3[68] = ~(p2[68] | p2[64]);
assign p3[69] = ~(p2[69] | p2[65]);
assign p3[70] = ~(p2[70] | p2[66]);
assign p3[71] = ~(p2[71] | p2[67]);
assign p3[72] = ~(p2[72] | p2[68]);
assign p3[73] = ~(p2[73] | p2[69]);
assign p3[74] = ~(p2[74] | p2[70]);
assign p3[75] = ~(p2[75] | p2[71]);
assign p3_CIN = ~(p2_CIN | p2[71]);
assign g4[00] = ~(g3[00] | (p3[00] & g3[68]));
assign g4[01] = ~(g3[01] | (p3[01] & g3[69]));
assign g4[02] = ~(g3[02] | (p3[02] & g3[70]));
assign g4[03] = ~(g3[03] | (p3[03] & g3[71]));
assign g4[04] = ~(g3[04] | (p3[04] & g3[72]));
assign g4[05] = ~(g3[05] | (p3[05] & g3[73]));
assign g4[06] = ~(g3[06] | (p3[06] & g3[74]));
assign g4[07] = ~(g3[07] | (p3[07] & g3_CIN));
assign g4[08] = ~(g3[08] | (p3[08] & g3[00]));
assign g4[09] = ~(g3[09] | (p3[09] & g3[01]));
assign g4[10] = ~(g3[10] | (p3[10] & g3[02]));
assign g4[11] = ~(g3[11] | (p3[11] & g3[03]));
assign g4[12] = ~(g3[12] | (p3[12] & g3[04]));
assign g4[13] = ~(g3[13] | (p3[13] & g3[05]));
assign g4[14] = ~(g3[14] | (p3[14] & g3[06]));
assign g4[15] = ~(g3[15] | (p3[15] & g3[07]));
assign g4[16] = ~(g3[16] | (p3[16] & g3[08]));
assign g4[17] = ~(g3[17] | (p3[17] & g3[09]));
assign g4[18] = ~(g3[18] | (p3[18] & g3[10]));
assign g4[19] = ~(g3[19] | (p3[19] & g3[11]));
assign g4[20] = ~(g3[20] | (p3[20] & g3[12]));
assign g4[21] = ~(g3[21] | (p3[21] & g3[13]));
assign g4[22] = ~(g3[22] | (p3[22] & g3[14]));
assign g4[23] = ~(g3[23] | (p3[23] & g3[15]));
assign g4[24] = ~(g3[24] | (p3[24] & g3[16]));
assign g4[25] = ~(g3[25] | (p3[25] & g3[17]));
assign g4[26] = ~(g3[26] | (p3[26] & g3[18]));
assign g4[27] = ~(g3[27] | (p3[27] & g3[19]));
assign g4[28] = ~(g3[28] | (p3[28] & g3[20]));
assign g4[29] = ~(g3[29] | (p3[29] & g3[21]));
assign g4[30] = ~(g3[30] | (p3[30] & g3[22]));
assign g4[31] = ~(g3[31] | (p3[31] & g3[23]));
assign g4[32] = ~(g3[32] | (p3[32] & g3[24]));
assign g4[33] = ~(g3[33] | (p3[33] & g3[25]));
assign g4[34] = ~(g3[34] | (p3[34] & g3[26]));
assign g4[35] = ~(g3[35] | (p3[35] & g3[27]));
assign g4[36] = ~(g3[36] | (p3[36] & g3[28]));
assign g4[37] = ~(g3[37] | (p3[37] & g3[29]));
assign g4[38] = ~(g3[38] | (p3[38] & g3[30]));
assign g4[39] = ~(g3[39] | (p3[39] & g3[31]));
assign g4[40] = ~(g3[40] | (p3[40] & g3[32]));
assign g4[41] = ~(g3[41] | (p3[41] & g3[33]));
assign g4[42] = ~(g3[42] | (p3[42] & g3[34]));
assign g4[43] = ~(g3[43] | (p3[43] & g3[35]));
assign g4[44] = ~(g3[44] | (p3[44] & g3[36]));
assign g4[45] = ~(g3[45] | (p3[45] & g3[37]));
assign g4[46] = ~(g3[46] | (p3[46] & g3[38]));
assign g4[47] = ~(g3[47] | (p3[47] & g3[39]));
assign g4[48] = ~(g3[48] | (p3[48] & g3[40]));
assign g4[49] = ~(g3[49] | (p3[49] & g3[41]));
assign g4[50] = ~(g3[50] | (p3[50] & g3[42]));
assign g4[51] = ~(g3[51] | (p3[51] & g3[43]));
assign g4[52] = ~(g3[52] | (p3[52] & g3[44]));
assign g4[53] = ~(g3[53] | (p3[53] & g3[45]));
assign g4[54] = ~(g3[54] | (p3[54] & g3[46]));
assign g4[55] = ~(g3[55] | (p3[55] & g3[47]));
assign g4[56] = ~(g3[56] | (p3[56] & g3[48]));
assign g4[57] = ~(g3[57] | (p3[57] & g3[49]));
assign g4[58] = ~(g3[58] | (p3[58] & g3[50]));
assign g4[59] = ~(g3[59] | (p3[59] & g3[51]));
assign g4[60] = ~(g3[60] | (p3[60] & g3[52]));
assign g4[61] = ~(g3[61] | (p3[61] & g3[53]));
assign g4[62] = ~(g3[62] | (p3[62] & g3[54]));
assign g4[63] = ~(g3[63] | (p3[63] & g3[55]));
assign g4[64] = ~(g3[64] | (p3[64] & g3[56]));
assign g4[65] = ~(g3[65] | (p3[65] & g3[57]));
assign g4[66] = ~(g3[66] | (p3[66] & g3[58]));
assign g4[67] = ~(g3[67] | (p3[67] & g3[59]));
assign g4[68] = ~(g3[68] | (p3[68] & g3[60]));
assign g4[69] = ~(g3[69] | (p3[69] & g3[61]));
assign g4[70] = ~(g3[70] | (p3[70] & g3[62]));
assign g4[71] = ~(g3[71] | (p3[71] & g3[63]));
assign g4[72] = ~(g3[72] | (p3[72] & g3[64]));
assign g4[73] = ~(g3[73] | (p3[73] & g3[65]));
assign g4[74] = ~(g3[74] | (p3[74] & g3[66]));
assign g4[75] = ~(g3[75] | (p3[75] & g3[67]));
assign g4_CIN = ~(g3_CIN | (p3_CIN & g3[67]));
assign p4[00] = ~(p3[00] & p3[68]);
assign p4[01] = ~(p3[01] & p3[69]);
assign p4[02] = ~(p3[02] & p3[70]);
assign p4[03] = ~(p3[03] & p3[71]);
assign p4[04] = ~(p3[04] & p3[72]);
assign p4[05] = ~(p3[05] & p3[73]);
assign p4[06] = ~(p3[06] & p3[74]);
assign p4[07] = ~(p3[07] & p3_CIN);
assign p4[08] = ~(p3[08] & p3[00]);
assign p4[09] = ~(p3[09] & p3[01]);
assign p4[10] = ~(p3[10] & p3[02]);
assign p4[11] = ~(p3[11] & p3[03]);
assign p4[12] = ~(p3[12] & p3[04]);
assign p4[13] = ~(p3[13] & p3[05]);
assign p4[14] = ~(p3[14] & p3[06]);
assign p4[15] = ~(p3[15] & p3[07]);
assign p4[16] = ~(p3[16] & p3[08]);
assign p4[17] = ~(p3[17] & p3[09]);
assign p4[18] = ~(p3[18] & p3[10]);
assign p4[19] = ~(p3[19] & p3[11]);
assign p4[20] = ~(p3[20] & p3[12]);
assign p4[21] = ~(p3[21] & p3[13]);
assign p4[22] = ~(p3[22] & p3[14]);
assign p4[23] = ~(p3[23] & p3[15]);
assign p4[24] = ~(p3[24] & p3[16]);
assign p4[25] = ~(p3[25] & p3[17]);
assign p4[26] = ~(p3[26] & p3[18]);
assign p4[27] = ~(p3[27] & p3[19]);
assign p4[28] = ~(p3[28] & p3[20]);
assign p4[29] = ~(p3[29] & p3[21]);
assign p4[30] = ~(p3[30] & p3[22]);
assign p4[31] = ~(p3[31] & p3[23]);
assign p4[32] = ~(p3[32] & p3[24]);
assign p4[33] = ~(p3[33] & p3[25]);
assign p4[34] = ~(p3[34] & p3[26]);
assign p4[35] = ~(p3[35] & p3[27]);
assign p4[36] = ~(p3[36] & p3[28]);
assign p4[37] = ~(p3[37] & p3[29]);
assign p4[38] = ~(p3[38] & p3[30]);
assign p4[39] = ~(p3[39] & p3[31]);
assign p4[40] = ~(p3[40] & p3[32]);
assign p4[41] = ~(p3[41] & p3[33]);
assign p4[42] = ~(p3[42] & p3[34]);
assign p4[43] = ~(p3[43] & p3[35]);
assign p4[44] = ~(p3[44] & p3[36]);
assign p4[45] = ~(p3[45] & p3[37]);
assign p4[46] = ~(p3[46] & p3[38]);
assign p4[47] = ~(p3[47] & p3[39]);
assign p4[48] = ~(p3[48] & p3[40]);
assign p4[49] = ~(p3[49] & p3[41]);
assign p4[50] = ~(p3[50] & p3[42]);
assign p4[51] = ~(p3[51] & p3[43]);
assign p4[52] = ~(p3[52] & p3[44]);
assign p4[53] = ~(p3[53] & p3[45]);
assign p4[54] = ~(p3[54] & p3[46]);
assign p4[55] = ~(p3[55] & p3[47]);
assign p4[56] = ~(p3[56] & p3[48]);
assign p4[57] = ~(p3[57] & p3[49]);
assign p4[58] = ~(p3[58] & p3[50]);
assign p4[59] = ~(p3[59] & p3[51]);
assign p4[60] = ~(p3[60] & p3[52]);
assign p4[61] = ~(p3[61] & p3[53]);
assign p4[62] = ~(p3[62] & p3[54]);
assign p4[63] = ~(p3[63] & p3[55]);
assign p4[64] = ~(p3[64] & p3[56]);
assign p4[65] = ~(p3[65] & p3[57]);
assign p4[66] = ~(p3[66] & p3[58]);
assign p4[67] = ~(p3[67] & p3[59]);
assign p4[68] = ~(p3[68] & p3[60]);
assign p4[69] = ~(p3[69] & p3[61]);
assign p4[70] = ~(p3[70] & p3[62]);
assign p4[71] = ~(p3[71] & p3[63]);
assign p4[72] = ~(p3[72] & p3[64]);
assign p4[73] = ~(p3[73] & p3[65]);
assign p4[74] = ~(p3[74] & p3[66]);
assign p4[75] = ~(p3[75] & p3[67]);
assign p4_CIN = ~(p3_CIN & p3[67]);
assign g5[00] = ~(g4[00] & (p4[00] | g4[60]));
assign g5[01] = ~(g4[01] & (p4[01] | g4[61]));
assign g5[02] = ~(g4[02] & (p4[02] | g4[62]));
assign g5[03] = ~(g4[03] & (p4[03] | g4[63]));
assign g5[04] = ~(g4[04] & (p4[04] | g4[64]));
assign g5[05] = ~(g4[05] & (p4[05] | g4[65]));
assign g5[06] = ~(g4[06] & (p4[06] | g4[66]));
assign g5[07] = ~(g4[07] & (p4[07] | g4[67]));
assign g5[08] = ~(g4[08] & (p4[08] | g4[68]));
assign g5[09] = ~(g4[09] & (p4[09] | g4[69]));
assign g5[10] = ~(g4[10] & (p4[10] | g4[70]));
assign g5[11] = ~(g4[11] & (p4[11] | g4[71]));
assign g5[12] = ~(g4[12] & (p4[12] | g4[72]));
assign g5[13] = ~(g4[13] & (p4[13] | g4[73]));
assign g5[14] = ~(g4[14] & (p4[14] | g4[74]));
assign g5[15] = ~(g4[15] & (p4[15] | g4_CIN));
assign g5[16] = ~(g4[16] & (p4[16] | g4[00]));
assign g5[17] = ~(g4[17] & (p4[17] | g4[01]));
assign g5[18] = ~(g4[18] & (p4[18] | g4[02]));
assign g5[19] = ~(g4[19] & (p4[19] | g4[03]));
assign g5[20] = ~(g4[20] & (p4[20] | g4[04]));
assign g5[21] = ~(g4[21] & (p4[21] | g4[05]));
assign g5[22] = ~(g4[22] & (p4[22] | g4[06]));
assign g5[23] = ~(g4[23] & (p4[23] | g4[07]));
assign g5[24] = ~(g4[24] & (p4[24] | g4[08]));
assign g5[25] = ~(g4[25] & (p4[25] | g4[09]));
assign g5[26] = ~(g4[26] & (p4[26] | g4[10]));
assign g5[27] = ~(g4[27] & (p4[27] | g4[11]));
assign g5[28] = ~(g4[28] & (p4[28] | g4[12]));
assign g5[29] = ~(g4[29] & (p4[29] | g4[13]));
assign g5[30] = ~(g4[30] & (p4[30] | g4[14]));
assign g5[31] = ~(g4[31] & (p4[31] | g4[15]));
assign g5[32] = ~(g4[32] & (p4[32] | g4[16]));
assign g5[33] = ~(g4[33] & (p4[33] | g4[17]));
assign g5[34] = ~(g4[34] & (p4[34] | g4[18]));
assign g5[35] = ~(g4[35] & (p4[35] | g4[19]));
assign g5[36] = ~(g4[36] & (p4[36] | g4[20]));
assign g5[37] = ~(g4[37] & (p4[37] | g4[21]));
assign g5[38] = ~(g4[38] & (p4[38] | g4[22]));
assign g5[39] = ~(g4[39] & (p4[39] | g4[23]));
assign g5[40] = ~(g4[40] & (p4[40] | g4[24]));
assign g5[41] = ~(g4[41] & (p4[41] | g4[25]));
assign g5[42] = ~(g4[42] & (p4[42] | g4[26]));
assign g5[43] = ~(g4[43] & (p4[43] | g4[27]));
assign g5[44] = ~(g4[44] & (p4[44] | g4[28]));
assign g5[45] = ~(g4[45] & (p4[45] | g4[29]));
assign g5[46] = ~(g4[46] & (p4[46] | g4[30]));
assign g5[47] = ~(g4[47] & (p4[47] | g4[31]));
assign g5[48] = ~(g4[48] & (p4[48] | g4[32]));
assign g5[49] = ~(g4[49] & (p4[49] | g4[33]));
assign g5[50] = ~(g4[50] & (p4[50] | g4[34]));
assign g5[51] = ~(g4[51] & (p4[51] | g4[35]));
assign g5[52] = ~(g4[52] & (p4[52] | g4[36]));
assign g5[53] = ~(g4[53] & (p4[53] | g4[37]));
assign g5[54] = ~(g4[54] & (p4[54] | g4[38]));
assign g5[55] = ~(g4[55] & (p4[55] | g4[39]));
assign g5[56] = ~(g4[56] & (p4[56] | g4[40]));
assign g5[57] = ~(g4[57] & (p4[57] | g4[41]));
assign g5[58] = ~(g4[58] & (p4[58] | g4[42]));
assign g5[59] = ~(g4[59] & (p4[59] | g4[43]));
assign g5[60] = ~(g4[60] & (p4[60] | g4[44]));
assign g5[61] = ~(g4[61] & (p4[61] | g4[45]));
assign g5[62] = ~(g4[62] & (p4[62] | g4[46]));
assign g5[63] = ~(g4[63] & (p4[63] | g4[47]));
assign g5[64] = ~(g4[64] & (p4[64] | g4[48]));
assign g5[65] = ~(g4[65] & (p4[65] | g4[49]));
assign g5[66] = ~(g4[66] & (p4[66] | g4[50]));
assign g5[67] = ~(g4[67] & (p4[67] | g4[51]));
assign g5[68] = ~(g4[68] & (p4[68] | g4[52]));
assign g5[69] = ~(g4[69] & (p4[69] | g4[53]));
assign g5[70] = ~(g4[70] & (p4[70] | g4[54]));
assign g5[71] = ~(g4[71] & (p4[71] | g4[55]));
assign g5[72] = ~(g4[72] & (p4[72] | g4[56]));
assign g5[73] = ~(g4[73] & (p4[73] | g4[57]));
assign g5[74] = ~(g4[74] & (p4[74] | g4[58]));
assign g5[75] = ~(g4[75] & (p4[75] | g4[59]));
assign g5_CIN = ~(g4_CIN & (p4_CIN | g4[59]));
assign p5[00] = ~(p4[00] | p4[60]);
assign p5[01] = ~(p4[01] | p4[61]);
assign p5[02] = ~(p4[02] | p4[62]);
assign p5[03] = ~(p4[03] | p4[63]);
assign p5[04] = ~(p4[04] | p4[64]);
assign p5[05] = ~(p4[05] | p4[65]);
assign p5[06] = ~(p4[06] | p4[66]);
assign p5[07] = ~(p4[07] | p4[67]);
assign p5[08] = ~(p4[08] | p4[68]);
assign p5[09] = ~(p4[09] | p4[69]);
assign p5[10] = ~(p4[10] | p4[70]);
assign p5[11] = ~(p4[11] | p4[71]);
assign p5[12] = ~(p4[12] | p4[72]);
assign p5[13] = ~(p4[13] | p4[73]);
assign p5[14] = ~(p4[14] | p4[74]);
assign p5[15] = ~(p4[15] | p4_CIN);
assign p5[16] = ~(p4[16] | p4[00]);
assign p5[17] = ~(p4[17] | p4[01]);
assign p5[18] = ~(p4[18] | p4[02]);
assign p5[19] = ~(p4[19] | p4[03]);
assign p5[20] = ~(p4[20] | p4[04]);
assign p5[21] = ~(p4[21] | p4[05]);
assign p5[22] = ~(p4[22] | p4[06]);
assign p5[23] = ~(p4[23] | p4[07]);
assign p5[24] = ~(p4[24] | p4[08]);
assign p5[25] = ~(p4[25] | p4[09]);
assign p5[26] = ~(p4[26] | p4[10]);
assign p5[27] = ~(p4[27] | p4[11]);
assign p5[28] = ~(p4[28] | p4[12]);
assign p5[29] = ~(p4[29] | p4[13]);
assign p5[30] = ~(p4[30] | p4[14]);
assign p5[31] = ~(p4[31] | p4[15]);
assign p5[32] = ~(p4[32] | p4[16]);
assign p5[33] = ~(p4[33] | p4[17]);
assign p5[34] = ~(p4[34] | p4[18]);
assign p5[35] = ~(p4[35] | p4[19]);
assign p5[36] = ~(p4[36] | p4[20]);
assign p5[37] = ~(p4[37] | p4[21]);
assign p5[38] = ~(p4[38] | p4[22]);
assign p5[39] = ~(p4[39] | p4[23]);
assign p5[40] = ~(p4[40] | p4[24]);
assign p5[41] = ~(p4[41] | p4[25]);
assign p5[42] = ~(p4[42] | p4[26]);
assign p5[43] = ~(p4[43] | p4[27]);
assign p5[44] = ~(p4[44] | p4[28]);
assign p5[45] = ~(p4[45] | p4[29]);
assign p5[46] = ~(p4[46] | p4[30]);
assign p5[47] = ~(p4[47] | p4[31]);
assign p5[48] = ~(p4[48] | p4[32]);
assign p5[49] = ~(p4[49] | p4[33]);
assign p5[50] = ~(p4[50] | p4[34]);
assign p5[51] = ~(p4[51] | p4[35]);
assign p5[52] = ~(p4[52] | p4[36]);
assign p5[53] = ~(p4[53] | p4[37]);
assign p5[54] = ~(p4[54] | p4[38]);
assign p5[55] = ~(p4[55] | p4[39]);
assign p5[56] = ~(p4[56] | p4[40]);
assign p5[57] = ~(p4[57] | p4[41]);
assign p5[58] = ~(p4[58] | p4[42]);
assign p5[59] = ~(p4[59] | p4[43]);
assign p5[60] = ~(p4[60] | p4[44]);
assign p5[61] = ~(p4[61] | p4[45]);
assign p5[62] = ~(p4[62] | p4[46]);
assign p5[63] = ~(p4[63] | p4[47]);
assign p5[64] = ~(p4[64] | p4[48]);
assign p5[65] = ~(p4[65] | p4[49]);
assign p5[66] = ~(p4[66] | p4[50]);
assign p5[67] = ~(p4[67] | p4[51]);
assign p5[68] = ~(p4[68] | p4[52]);
assign p5[69] = ~(p4[69] | p4[53]);
assign p5[70] = ~(p4[70] | p4[54]);
assign p5[71] = ~(p4[71] | p4[55]);
assign p5[72] = ~(p4[72] | p4[56]);
assign p5[73] = ~(p4[73] | p4[57]);
assign p5[74] = ~(p4[74] | p4[58]);
assign p5[75] = ~(p4[75] | p4[59]);
assign p5_CIN = ~(p4_CIN | p4[59]);
assign g6[00] = ~(g5[00] | (p5[00] & g5[44]));
assign g6[01] = ~(g5[01] | (p5[01] & g5[45]));
assign g6[02] = ~(g5[02] | (p5[02] & g5[46]));
assign g6[03] = ~(g5[03] | (p5[03] & g5[47]));
assign g6[04] = ~(g5[04] | (p5[04] & g5[48]));
assign g6[05] = ~(g5[05] | (p5[05] & g5[49]));
assign g6[06] = ~(g5[06] | (p5[06] & g5[50]));
assign g6[07] = ~(g5[07] | (p5[07] & g5[51]));
assign g6[08] = ~(g5[08] | (p5[08] & g5[52]));
assign g6[09] = ~(g5[09] | (p5[09] & g5[53]));
assign g6[10] = ~(g5[10] | (p5[10] & g5[54]));
assign g6[11] = ~(g5[11] | (p5[11] & g5[55]));
assign g6[12] = ~(g5[12] | (p5[12] & g5[56]));
assign g6[13] = ~(g5[13] | (p5[13] & g5[57]));
assign g6[14] = ~(g5[14] | (p5[14] & g5[58]));
assign g6[15] = ~(g5[15] | (p5[15] & g5[59]));
assign g6[16] = ~(g5[16] | (p5[16] & g5[60]));
assign g6[17] = ~(g5[17] | (p5[17] & g5[61]));
assign g6[18] = ~(g5[18] | (p5[18] & g5[62]));
assign g6[19] = ~(g5[19] | (p5[19] & g5[63]));
assign g6[20] = ~(g5[20] | (p5[20] & g5[64]));
assign g6[21] = ~(g5[21] | (p5[21] & g5[65]));
assign g6[22] = ~(g5[22] | (p5[22] & g5[66]));
assign g6[23] = ~(g5[23] | (p5[23] & g5[67]));
assign g6[24] = ~(g5[24] | (p5[24] & g5[68]));
assign g6[25] = ~(g5[25] | (p5[25] & g5[69]));
assign g6[26] = ~(g5[26] | (p5[26] & g5[70]));
assign g6[27] = ~(g5[27] | (p5[27] & g5[71]));
assign g6[28] = ~(g5[28] | (p5[28] & g5[72]));
assign g6[29] = ~(g5[29] | (p5[29] & g5[73]));
assign g6[30] = ~(g5[30] | (p5[30] & g5[74]));
assign g6[31] = ~(g5[31] | (p5[31] & g5_CIN));
assign g6[32] = ~(g5[32] | (p5[32] & g5[00]));
assign g6[33] = ~(g5[33] | (p5[33] & g5[01]));
assign g6[34] = ~(g5[34] | (p5[34] & g5[02]));
assign g6[35] = ~(g5[35] | (p5[35] & g5[03]));
assign g6[36] = ~(g5[36] | (p5[36] & g5[04]));
assign g6[37] = ~(g5[37] | (p5[37] & g5[05]));
assign g6[38] = ~(g5[38] | (p5[38] & g5[06]));
assign g6[39] = ~(g5[39] | (p5[39] & g5[07]));
assign g6[40] = ~(g5[40] | (p5[40] & g5[08]));
assign g6[41] = ~(g5[41] | (p5[41] & g5[09]));
assign g6[42] = ~(g5[42] | (p5[42] & g5[10]));
assign g6[43] = ~(g5[43] | (p5[43] & g5[11]));
assign g6[44] = ~(g5[44] | (p5[44] & g5[12]));
assign g6[45] = ~(g5[45] | (p5[45] & g5[13]));
assign g6[46] = ~(g5[46] | (p5[46] & g5[14]));
assign g6[47] = ~(g5[47] | (p5[47] & g5[15]));
assign g6[48] = ~(g5[48] | (p5[48] & g5[16]));
assign g6[49] = ~(g5[49] | (p5[49] & g5[17]));
assign g6[50] = ~(g5[50] | (p5[50] & g5[18]));
assign g6[51] = ~(g5[51] | (p5[51] & g5[19]));
assign g6[52] = ~(g5[52] | (p5[52] & g5[20]));
assign g6[53] = ~(g5[53] | (p5[53] & g5[21]));
assign g6[54] = ~(g5[54] | (p5[54] & g5[22]));
assign g6[55] = ~(g5[55] | (p5[55] & g5[23]));
assign g6[56] = ~(g5[56] | (p5[56] & g5[24]));
assign g6[57] = ~(g5[57] | (p5[57] & g5[25]));
assign g6[58] = ~(g5[58] | (p5[58] & g5[26]));
assign g6[59] = ~(g5[59] | (p5[59] & g5[27]));
assign g6[60] = ~(g5[60] | (p5[60] & g5[28]));
assign g6[61] = ~(g5[61] | (p5[61] & g5[29]));
assign g6[62] = ~(g5[62] | (p5[62] & g5[30]));
assign g6[63] = ~(g5[63] | (p5[63] & g5[31]));
assign g6[64] = ~(g5[64] | (p5[64] & g5[32]));
assign g6[65] = ~(g5[65] | (p5[65] & g5[33]));
assign g6[66] = ~(g5[66] | (p5[66] & g5[34]));
assign g6[67] = ~(g5[67] | (p5[67] & g5[35]));
assign g6[68] = ~(g5[68] | (p5[68] & g5[36]));
assign g6[69] = ~(g5[69] | (p5[69] & g5[37]));
assign g6[70] = ~(g5[70] | (p5[70] & g5[38]));
assign g6[71] = ~(g5[71] | (p5[71] & g5[39]));
assign g6[72] = ~(g5[72] | (p5[72] & g5[40]));
assign g6[73] = ~(g5[73] | (p5[73] & g5[41]));
assign g6[74] = ~(g5[74] | (p5[74] & g5[42]));
assign g6[75] = ~(g5[75] | (p5[75] & g5[43]));
assign g6_CIN = ~(g5_CIN | (p5_CIN & g5[43]));
assign p6[00] = ~(p5[00] & p5[44]);
assign p6[01] = ~(p5[01] & p5[45]);
assign p6[02] = ~(p5[02] & p5[46]);
assign p6[03] = ~(p5[03] & p5[47]);
assign p6[04] = ~(p5[04] & p5[48]);
assign p6[05] = ~(p5[05] & p5[49]);
assign p6[06] = ~(p5[06] & p5[50]);
assign p6[07] = ~(p5[07] & p5[51]);
assign p6[08] = ~(p5[08] & p5[52]);
assign p6[09] = ~(p5[09] & p5[53]);
assign p6[10] = ~(p5[10] & p5[54]);
assign p6[11] = ~(p5[11] & p5[55]);
assign p6[12] = ~(p5[12] & p5[56]);
assign p6[13] = ~(p5[13] & p5[57]);
assign p6[14] = ~(p5[14] & p5[58]);
assign p6[15] = ~(p5[15] & p5[59]);
assign p6[16] = ~(p5[16] & p5[60]);
assign p6[17] = ~(p5[17] & p5[61]);
assign p6[18] = ~(p5[18] & p5[62]);
assign p6[19] = ~(p5[19] & p5[63]);
assign p6[20] = ~(p5[20] & p5[64]);
assign p6[21] = ~(p5[21] & p5[65]);
assign p6[22] = ~(p5[22] & p5[66]);
assign p6[23] = ~(p5[23] & p5[67]);
assign p6[24] = ~(p5[24] & p5[68]);
assign p6[25] = ~(p5[25] & p5[69]);
assign p6[26] = ~(p5[26] & p5[70]);
assign p6[27] = ~(p5[27] & p5[71]);
assign p6[28] = ~(p5[28] & p5[72]);
assign p6[29] = ~(p5[29] & p5[73]);
assign p6[30] = ~(p5[30] & p5[74]);
assign p6[31] = ~(p5[31] & p5_CIN);
assign p6[32] = ~(p5[32] & p5[00]);
assign p6[33] = ~(p5[33] & p5[01]);
assign p6[34] = ~(p5[34] & p5[02]);
assign p6[35] = ~(p5[35] & p5[03]);
assign p6[36] = ~(p5[36] & p5[04]);
assign p6[37] = ~(p5[37] & p5[05]);
assign p6[38] = ~(p5[38] & p5[06]);
assign p6[39] = ~(p5[39] & p5[07]);
assign p6[40] = ~(p5[40] & p5[08]);
assign p6[41] = ~(p5[41] & p5[09]);
assign p6[42] = ~(p5[42] & p5[10]);
assign p6[43] = ~(p5[43] & p5[11]);
assign p6[44] = ~(p5[44] & p5[12]);
assign p6[45] = ~(p5[45] & p5[13]);
assign p6[46] = ~(p5[46] & p5[14]);
assign p6[47] = ~(p5[47] & p5[15]);
assign p6[48] = ~(p5[48] & p5[16]);
assign p6[49] = ~(p5[49] & p5[17]);
assign p6[50] = ~(p5[50] & p5[18]);
assign p6[51] = ~(p5[51] & p5[19]);
assign p6[52] = ~(p5[52] & p5[20]);
assign p6[53] = ~(p5[53] & p5[21]);
assign p6[54] = ~(p5[54] & p5[22]);
assign p6[55] = ~(p5[55] & p5[23]);
assign p6[56] = ~(p5[56] & p5[24]);
assign p6[57] = ~(p5[57] & p5[25]);
assign p6[58] = ~(p5[58] & p5[26]);
assign p6[59] = ~(p5[59] & p5[27]);
assign p6[60] = ~(p5[60] & p5[28]);
assign p6[61] = ~(p5[61] & p5[29]);
assign p6[62] = ~(p5[62] & p5[30]);
assign p6[63] = ~(p5[63] & p5[31]);
assign p6[64] = ~(p5[64] & p5[32]);
assign p6[65] = ~(p5[65] & p5[33]);
assign p6[66] = ~(p5[66] & p5[34]);
assign p6[67] = ~(p5[67] & p5[35]);
assign p6[68] = ~(p5[68] & p5[36]);
assign p6[69] = ~(p5[69] & p5[37]);
assign p6[70] = ~(p5[70] & p5[38]);
assign p6[71] = ~(p5[71] & p5[39]);
assign p6[72] = ~(p5[72] & p5[40]);
assign p6[73] = ~(p5[73] & p5[41]);
assign p6[74] = ~(p5[74] & p5[42]);
assign p6[75] = ~(p5[75] & p5[43]);
assign p6_CIN = ~(p5_CIN & p5[43]);
assign g7[00] = ~(g6[00] & (p6[00] | g6[12]));
assign g7[01] = ~(g6[01] & (p6[01] | g6[13]));
assign g7[02] = ~(g6[02] & (p6[02] | g6[14]));
assign g7[03] = ~(g6[03] & (p6[03] | g6[15]));
assign g7[04] = ~(g6[04] & (p6[04] | g6[16]));
assign g7[05] = ~(g6[05] & (p6[05] | g6[17]));
assign g7[06] = ~(g6[06] & (p6[06] | g6[18]));
assign g7[07] = ~(g6[07] & (p6[07] | g6[19]));
assign g7[08] = ~(g6[08] & (p6[08] | g6[20]));
assign g7[09] = ~(g6[09] & (p6[09] | g6[21]));
assign g7[10] = ~(g6[10] & (p6[10] | g6[22]));
assign g7[11] = ~(g6[11] & (p6[11] | g6[23]));
assign g7[12] = ~(g6[12] & (p6[12] | g6[24]));
assign g7[13] = ~(g6[13] & (p6[13] | g6[25]));
assign g7[14] = ~(g6[14] & (p6[14] | g6[26]));
assign g7[15] = ~(g6[15] & (p6[15] | g6[27]));
assign g7[16] = ~(g6[16] & (p6[16] | g6[28]));
assign g7[17] = ~(g6[17] & (p6[17] | g6[29]));
assign g7[18] = ~(g6[18] & (p6[18] | g6[30]));
assign g7[19] = ~(g6[19] & (p6[19] | g6[31]));
assign g7[20] = ~(g6[20] & (p6[20] | g6[32]));
assign g7[21] = ~(g6[21] & (p6[21] | g6[33]));
assign g7[22] = ~(g6[22] & (p6[22] | g6[34]));
assign g7[23] = ~(g6[23] & (p6[23] | g6[35]));
assign g7[24] = ~(g6[24] & (p6[24] | g6[36]));
assign g7[25] = ~(g6[25] & (p6[25] | g6[37]));
assign g7[26] = ~(g6[26] & (p6[26] | g6[38]));
assign g7[27] = ~(g6[27] & (p6[27] | g6[39]));
assign g7[28] = ~(g6[28] & (p6[28] | g6[40]));
assign g7[29] = ~(g6[29] & (p6[29] | g6[41]));
assign g7[30] = ~(g6[30] & (p6[30] | g6[42]));
assign g7[31] = ~(g6[31] & (p6[31] | g6[43]));
assign g7[32] = ~(g6[32] & (p6[32] | g6[44]));
assign g7[33] = ~(g6[33] & (p6[33] | g6[45]));
assign g7[34] = ~(g6[34] & (p6[34] | g6[46]));
assign g7[35] = ~(g6[35] & (p6[35] | g6[47]));
assign g7[36] = ~(g6[36] & (p6[36] | g6[48]));
assign g7[37] = ~(g6[37] & (p6[37] | g6[49]));
assign g7[38] = ~(g6[38] & (p6[38] | g6[50]));
assign g7[39] = ~(g6[39] & (p6[39] | g6[51]));
assign g7[40] = ~(g6[40] & (p6[40] | g6[52]));
assign g7[41] = ~(g6[41] & (p6[41] | g6[53]));
assign g7[42] = ~(g6[42] & (p6[42] | g6[54]));
assign g7[43] = ~(g6[43] & (p6[43] | g6[55]));
assign g7[44] = ~(g6[44] & (p6[44] | g6[56]));
assign g7[45] = ~(g6[45] & (p6[45] | g6[57]));
assign g7[46] = ~(g6[46] & (p6[46] | g6[58]));
assign g7[47] = ~(g6[47] & (p6[47] | g6[59]));
assign g7[48] = ~(g6[48] & (p6[48] | g6[60]));
assign g7[49] = ~(g6[49] & (p6[49] | g6[61]));
assign g7[50] = ~(g6[50] & (p6[50] | g6[62]));
assign g7[51] = ~(g6[51] & (p6[51] | g6[63]));
assign g7[52] = ~(g6[52] & (p6[52] | g6[64]));
assign g7[53] = ~(g6[53] & (p6[53] | g6[65]));
assign g7[54] = ~(g6[54] & (p6[54] | g6[66]));
assign g7[55] = ~(g6[55] & (p6[55] | g6[67]));
assign g7[56] = ~(g6[56] & (p6[56] | g6[68]));
assign g7[57] = ~(g6[57] & (p6[57] | g6[69]));
assign g7[58] = ~(g6[58] & (p6[58] | g6[70]));
assign g7[59] = ~(g6[59] & (p6[59] | g6[71]));
assign g7[60] = ~(g6[60] & (p6[60] | g6[72]));
assign g7[61] = ~(g6[61] & (p6[61] | g6[73]));
assign g7[62] = ~(g6[62] & (p6[62] | g6[74]));
assign g7[63] = ~(g6[63] & (p6[63] | g6_CIN));
assign g7[64] = ~(g6[64] & (p6[64] | g6[00]));
assign g7[65] = ~(g6[65] & (p6[65] | g6[01]));
assign g7[66] = ~(g6[66] & (p6[66] | g6[02]));
assign g7[67] = ~(g6[67] & (p6[67] | g6[03]));
assign g7[68] = ~(g6[68] & (p6[68] | g6[04]));
assign g7[69] = ~(g6[69] & (p6[69] | g6[05]));
assign g7[70] = ~(g6[70] & (p6[70] | g6[06]));
assign g7[71] = ~(g6[71] & (p6[71] | g6[07]));
assign g7[72] = ~(g6[72] & (p6[72] | g6[08]));
assign g7[73] = ~(g6[73] & (p6[73] | g6[09]));
assign g7_75  = ~(g6[75] & (p6[75] | g6[11]));
assign g7_CIN = ~(g6_CIN & (p6_CIN | g6[11]));
assign p      = ~(p6[75] | p6[63]);
assign g      =   g7_75;
assign inv    = ~(subn | g7_75);
assign dmr[00] = ~(g7_CIN ^ x[00] ^ inv);
assign dmr[01] = ~(g7[00] ^ x[01] ^ inv);
assign dmr[02] = ~(g7[01] ^ x[02] ^ inv);
assign dmr[03] = ~(g7[02] ^ x[03] ^ inv);
assign dmr[04] = ~(g7[03] ^ x[04] ^ inv);
assign dmr[05] = ~(g7[04] ^ x[05] ^ inv);
assign dmr[06] = ~(g7[05] ^ x[06] ^ inv);
assign dmr[07] = ~(g7[06] ^ x[07] ^ inv);
assign dmr[08] = ~(g7[07] ^ x[08] ^ inv);
assign dmr[09] = ~(g7[08] ^ x[09] ^ inv);
assign dmr[10] = ~(g7[09] ^ x[10] ^ inv);
assign dmr[11] = ~(g7[10] ^ x[11] ^ inv);
assign dmr[12] = ~(g7[11] ^ x[12] ^ inv);
assign dmr[13] = ~(g7[12] ^ x[13] ^ inv);
assign dmr[14] = ~(g7[13] ^ x[14] ^ inv);
assign dmr[15] = ~(g7[14] ^ x[15] ^ inv);
assign dmr[16] = ~(g7[15] ^ x[16] ^ inv);
assign dmr[17] = ~(g7[16] ^ x[17] ^ inv);
assign dmr[18] = ~(g7[17] ^ x[18] ^ inv);
assign dmr[19] = ~(g7[18] ^ x[19] ^ inv);
assign dmr[20] = ~(g7[19] ^ x[20] ^ inv);
assign dmr[21] = ~(g7[20] ^ x[21] ^ inv);
assign dmr[22] = ~(g7[21] ^ x[22] ^ inv);
assign dmr[23] = ~(g7[22] ^ x[23] ^ inv);
assign dmr[24] = ~(g7[23] ^ x[24] ^ inv);
assign dmr[25] = ~(g7[24] ^ x[25] ^ inv);
assign dmr[26] = ~(g7[25] ^ x[26] ^ inv);
assign dmr[27] = ~(g7[26] ^ x[27] ^ inv);
assign dmr[28] = ~(g7[27] ^ x[28] ^ inv);
assign dmr[29] = ~(g7[28] ^ x[29] ^ inv);
assign dmr[30] = ~(g7[29] ^ x[30] ^ inv);
assign dmr[31] = ~(g7[30] ^ x[31] ^ inv);
assign dmr[32] = ~(g7[31] ^ x[32] ^ inv);
assign dmr[33] = ~(g7[32] ^ x[33] ^ inv);
assign dmr[34] = ~(g7[33] ^ x[34] ^ inv);
assign dmr[35] = ~(g7[34] ^ x[35] ^ inv);
assign dmr[36] = ~(g7[35] ^ x[36] ^ inv);
assign dmr[37] = ~(g7[36] ^ x[37] ^ inv);
assign dmr[38] = ~(g7[37] ^ x[38] ^ inv);
assign dmr[39] = ~(g7[38] ^ x[39] ^ inv);
assign dmr[40] = ~(g7[39] ^ x[40] ^ inv);
assign dmr[41] = ~(g7[40] ^ x[41] ^ inv);
assign dmr[42] = ~(g7[41] ^ x[42] ^ inv);
assign dmr[43] = ~(g7[42] ^ x[43] ^ inv);
assign dmr[44] = ~(g7[43] ^ x[44] ^ inv);
assign dmr[45] = ~(g7[44] ^ x[45] ^ inv);
assign dmr[46] = ~(g7[45] ^ x[46] ^ inv);
assign dmr[47] = ~(g7[46] ^ x[47] ^ inv);
assign dmr[48] = ~(g7[47] ^ x[48] ^ inv);
assign dmr[49] = ~(g7[48] ^ x[49] ^ inv);
assign dmr[50] = ~(g7[49] ^ x[50] ^ inv);
assign dmr[51] = ~(g7[50] ^ x[51] ^ inv);
assign dmr[52] = ~(g7[51] ^ x[52] ^ inv);
assign dmr[53] = ~(g7[52] ^ x[53] ^ inv);
assign dmr[54] = ~(g7[53] ^ x[54] ^ inv);
assign dmr[55] = ~(g7[54] ^ x[55] ^ inv);
assign dmr[56] = ~(g7[55] ^ x[56] ^ inv);
assign dmr[57] = ~(g7[56] ^ x[57] ^ inv);
assign dmr[58] = ~(g7[57] ^ x[58] ^ inv);
assign dmr[59] = ~(g7[58] ^ x[59] ^ inv);
assign dmr[60] = ~(g7[59] ^ x[60] ^ inv);
assign dmr[61] = ~(g7[60] ^ x[61] ^ inv);
assign dmr[62] = ~(g7[61] ^ x[62] ^ inv);
assign dmr[63] = ~(g7[62] ^ x[63] ^ inv);
assign dmr[64] = ~(g7[63] ^ x[64] ^ inv);
assign dmr[65] = ~(g7[64] ^ x[65] ^ inv);
assign dmr[66] = ~(g7[65] ^ x[66] ^ inv);
assign dmr[67] = ~(g7[66] ^ x[67] ^ inv);
assign dmr[68] = ~(g7[67] ^ x[68] ^ inv);
assign dmr[69] = ~(g7[68] ^ x[69] ^ inv);
assign dmr[70] = ~(g7[69] ^ x[70] ^ inv);
assign dmr[71] = ~(g7[70] ^ x[71] ^ inv);
assign dmr[72] = ~(g7[71] ^ x[72] ^ inv);
assign dmr[73] = ~(g7[72] ^ x[73] ^ inv);
assign dmr[74] = ~(g7[73] ^ x[74] ^ inv);
wire unused_p6_74 = p6[74];
endmodule
module fpmlzp (nsc, ms, mc, co, sub, fpop);
output  [6:0] nsc;
input  [74:0] ms;
input  [75:1] mc;
input         co;
input         sub;
input         fpop;
wire  [6:0] nsc0;
wire  [6:0] nsc1;
wire [75:1] sn;
wire [74:0] p;
wire [74:0] gn;
wire [75:1] f0;
wire [75:1] f1;
assign
    sn[75:1]  = ~({~sub,ms[74:1]} ^  mc[75:1]),
    p[74:0]   =  (ms[74:0] | {mc[74:1],1'b0}),
    gn[74:0]  = ~(ms[74:0] & {mc[74:1],1'b0}),
    f0[75:1]  = sub ? sn[75:1] & gn[74:0] : p[74:0],
    f1[75:1]  =       sn[75:1] & p[74:0];
fpmlzc fpmlzc_0 (.cnt(nsc0[6:0]), .a({f0[75:1],1'b1}));
fpmlzc fpmlzc_1 (.cnt(nsc1[6:0]), .a({f1[75:1],1'b1}));
assign nsc[6:0] = ~fpop  ? 7'd51
                :  co    ? nsc1[6:0]
                :          nsc0[6:0];
endmodule
module fpmlzc (cnt, a);
output  [6:0] cnt;
input  [75:0] a;
assign cnt[6:0] = a[75] ? 7'd 0
                : a[74] ? 7'd 1
                : a[73] ? 7'd 2
                : a[72] ? 7'd 3
                : a[71] ? 7'd 4
                : a[70] ? 7'd 5
                : a[69] ? 7'd 6
                : a[68] ? 7'd 7
                : a[67] ? 7'd 8
                : a[66] ? 7'd 9
                : a[65] ? 7'd 10
                : a[64] ? 7'd 11
                : a[63] ? 7'd 12
                : a[62] ? 7'd 13
                : a[61] ? 7'd 14
                : a[60] ? 7'd 15
                : a[59] ? 7'd 16
                : a[58] ? 7'd 17
                : a[57] ? 7'd 18
                : a[56] ? 7'd 19
                : a[55] ? 7'd 20
                : a[54] ? 7'd 21
                : a[53] ? 7'd 22
                : a[52] ? 7'd 23
                : a[51] ? 7'd 24
                : a[50] ? 7'd 25
                : a[49] ? 7'd 26
                : a[48] ? 7'd 27
                : a[47] ? 7'd 28
                : a[46] ? 7'd 29
                : a[45] ? 7'd 30
                : a[44] ? 7'd 31
                : a[43] ? 7'd 32
                : a[42] ? 7'd 33
                : a[41] ? 7'd 34
                : a[40] ? 7'd 35
                : a[39] ? 7'd 36
                : a[38] ? 7'd 37
                : a[37] ? 7'd 38
                : a[36] ? 7'd 39
                : a[35] ? 7'd 40
                : a[34] ? 7'd 41
                : a[33] ? 7'd 42
                : a[32] ? 7'd 43
                : a[31] ? 7'd 44
                : a[30] ? 7'd 45
                : a[29] ? 7'd 46
                : a[28] ? 7'd 47
                : a[27] ? 7'd 48
                : a[26] ? 7'd 49
                : a[25] ? 7'd 50
                : a[24] ? 7'd 51
                : a[23] ? 7'd 52
                : a[22] ? 7'd 53
                : a[21] ? 7'd 54
                : a[20] ? 7'd 55
                : a[19] ? 7'd 56
                : a[18] ? 7'd 57
                : a[17] ? 7'd 58
                : a[16] ? 7'd 59
                : a[15] ? 7'd 60
                : a[14] ? 7'd 61
                : a[13] ? 7'd 62
                : a[12] ? 7'd 63
                : a[11] ? 7'd 64
                : a[10] ? 7'd 65
                : a[09] ? 7'd 66
                : a[08] ? 7'd 67
                : a[07] ? 7'd 68
                : a[06] ? 7'd 69
                : a[05] ? 7'd 70
                : a[04] ? 7'd 71
                : a[03] ? 7'd 72
                : a[02] ? 7'd 73
                : a[01] ? 7'd 74
                : a[00] ? 7'd 75
                :         7'd 127;
endmodule
module fpme3 (
   e2i, mrz, rsgn,
   e2, sub, dmrg, dmrp, msgn, csgn, rm1
);
output  [9:0] e2i;
output        mrz;
output        rsgn;
input   [9:0] e2;
input         sub;
input         dmrg;
input         dmrp;
input         msgn;
input         csgn;
input         rm1;
wire          nsgn;
assign
   e2i[9:0] = e2[9:0] + 10'b1,
   mrz = sub & dmrp & ~dmrg,
   nsgn = (dmrg | ~sub) ? msgn : csgn,
   rsgn = mrz ? rm1 : nsgn;
endmodule
module fpmnsh (nmr, sadj, dmr, nsc, stki, fpop);
output [33:0] nmr;
output        sadj;
input  [74:0] dmr;
input   [6:0] nsc;
input         stki;
input         fpop;
wire [81:17] s0;
wire  [64:0] s1;
wire  [48:0] s2;
wire  [40:0] s3;
wire  [36:0] s4;
wire  [34:0] s5;
wire  [33:0] s6;
wire   [3:0] en;
wire         sadj;
assign
   s0[81:17] = ~(nsc[6] ?  {dmr[19:0],45'b0} : {9'b0,dmr[74:19]}),
   s1[64:00] = ~(nsc[5] ? ~{dmr[51:0],13'b0} : s0[81:17]),
   s2[48:00] = ~(nsc[4] ? s1[48:0]           : s1[64:16]),
   s3[40:00] = ~(nsc[3] ? s2[40:0]           : s2[48:08]),
   s4[36:00] = ~(nsc[2] ? s3[36:0]           : s3[40:04]),
   s5[34:00] = ~(nsc[1] ? s4[34:0]           : s4[36:02]),
   s6[33:00] = ~(nsc[0] ? s5[33:0]           : s5[34:01]);
assign
   en[0] = ~nsc[1] & ~nsc[0] & fpop,
   en[1] = ~nsc[1] &  nsc[0] & fpop,
   en[2] =  nsc[1] & ~nsc[0] & fpop,
   en[3] =  nsc[1] &  nsc[0] & fpop,
   sadj  = ( (en[0] & s4[28])
           | (en[1] & s4[27])
           | (en[2] & s4[26])
           | (en[3] & s4[25])
           );
assign
   nmr[33:1] = ~(sadj ? s6[32:0] : s6[33:1]),
   nmr[0]    = ~(~stki & (sadj | s6[0]));
endmodule
module fpmnsk (stky, dmr, nsc);
output        stky;
input  [49:0] dmr;
input   [6:0] nsc;
reg           stky;
always @(nsc or dmr)
   casez (nsc[6:0])
   7'd0 :   stky = |dmr[49:0];
   7'd1 :   stky = |dmr[48:0];
   7'd2 :   stky = |dmr[47:0];
   7'd3 :   stky = |dmr[46:0];
   7'd4 :   stky = |dmr[45:0];
   7'd5 :   stky = |dmr[44:0];
   7'd6 :   stky = |dmr[43:0];
   7'd7 :   stky = |dmr[42:0];
   7'd8 :   stky = |dmr[41:0];
   7'd9 :   stky = |dmr[40:0];
   7'd10:   stky = |dmr[39:0];
   7'd11:   stky = |dmr[38:0];
   7'd12:   stky = |dmr[37:0];
   7'd13:   stky = |dmr[36:0];
   7'd14:   stky = |dmr[35:0];
   7'd15:   stky = |dmr[34:0];
   7'd16:   stky = |dmr[33:0];
   7'd17:   stky = |dmr[32:0];
   7'd18:   stky = |dmr[31:0];
   7'd19:   stky = |dmr[30:0];
   7'd20:   stky = |dmr[29:0];
   7'd21:   stky = |dmr[28:0];
   7'd22:   stky = |dmr[27:0];
   7'd23:   stky = |dmr[26:0];
   7'd24:   stky = |dmr[25:0];
   7'd25:   stky = |dmr[24:0];
   7'd26:   stky = |dmr[23:0];
   7'd27:   stky = |dmr[22:0];
   7'd28:   stky = |dmr[21:0];
   7'd29:   stky = |dmr[20:0];
   7'd30:   stky = |dmr[19:0];
   7'd31:   stky = |dmr[18:0];
   7'd32:   stky = |dmr[17:0];
   7'd33:   stky = |dmr[16:0];
   7'd34:   stky = |dmr[15:0];
   7'd35:   stky = |dmr[14:0];
   7'd36:   stky = |dmr[13:0];
   7'd37:   stky = |dmr[12:0];
   7'd38:   stky = |dmr[11:0];
   7'd39:   stky = |dmr[10:0];
   7'd40:   stky = |dmr[09:0];
   7'd41:   stky = |dmr[08:0];
   7'd42:   stky = |dmr[07:0];
   7'd43:   stky = |dmr[06:0];
   7'd44:   stky = |dmr[05:0];
   7'd45:   stky = |dmr[04:0];
   7'd46:   stky = |dmr[03:0];
   7'd47:   stky = |dmr[02:0];
   7'd48:   stky = |dmr[01:0];
   7'd49:   stky =  dmr[00];
   7'd50:   stky = 1'b0;
   7'd51:   stky = 1'b0;
   7'd52:   stky = 1'b0;
   7'd53:   stky = 1'b0;
   7'd54:   stky = 1'b0;
   7'd55:   stky = 1'b0;
   7'd56:   stky = 1'b0;
   7'd57:   stky = 1'b0;
   7'd58:   stky = 1'b0;
   7'd59:   stky = 1'b0;
   7'd60:   stky = 1'b0;
   7'd61:   stky = 1'b0;
   7'd62:   stky = 1'b0;
   7'd63:   stky = 1'b0;
   7'd64:   stky = 1'b0;
   7'd65:   stky = 1'b0;
   7'd66:   stky = 1'b0;
   7'd67:   stky = 1'b0;
   7'd68:   stky = 1'b0;
   7'd69:   stky = 1'b0;
   7'd70:   stky = 1'b0;
   7'd71:   stky = 1'b0;
   7'd72:   stky = 1'b0;
   7'd73:   stky = 1'b0;
   7'd74:   stky = 1'b0;
   7'd75:   stky = 1'b0;
   default: stky = 1'bx;
   endcase
endmodule
module fpme4 (
   e2m1, e2p0, e2p1, ovfm1, ovfp0, ovfp1,
   unfnm1, unfnp0, unfnp1, ezm1, ezp0, mskm1, mskp0, mskp1,
   e2, e2i, nsc, mrz, infsel
);
output  [9:0] e2m1;
output  [9:0] e2p0;
output  [9:0] e2p1;
output        ovfm1;
output        ovfp0;
output        ovfp1;
output        unfnm1;
output        unfnp0;
output        unfnp1;
output        ezm1;
output        ezp0;
output  [3:0] mskm1;
output  [3:0] mskp0;
output  [3:0] mskp1;
input   [9:0] e2;
input   [9:0] e2i;
input   [6:0] nsc;
input         mrz;
input         infsel;
wire          ovfunfm1;
wire          ovfunfp0;
wire          ovfunfp1;
assign
   e2m1[9:0] = e2[9:0]  + {3'b111, ~nsc[6:0]},
   e2p0[9:0] = e2i[9:0] + {3'b111, ~nsc[6:0]},
   e2p1[9:0] = e2i[9:0] + {3'b111, ~nsc[6:0]} + 1;
assign
   ovfunfm1 = (~e2m1[9] & (e2m1[8:0] >= 9'hff))
            | ( e2m1[9] | (e2m1[8:0] == 9'h00)),
   ovfunfp0 = (~e2p0[9] & (e2p0[8:0] >= 9'hff))
            | ( e2p0[9] | (e2p0[8:0] == 9'h00)),
   ovfunfp1 = (~e2p1[9] & (e2p1[8:0] >= 9'hff))
            | ( e2p1[9] | (e2p1[8:0] == 9'h00)),
   ovfm1    = (~e2m1[9] & (e2m1[8:0] >= 9'hff)),
   ovfp0    = (~e2p0[9] & (e2p0[8:0] >= 9'hff)),
   ovfp1    = (~e2p1[9] & (e2p1[8:0] >= 9'hff)),
   unfnm1   = ~(e2m1[9] | (e2m1[8:0] == 9'h00)),
   unfnp0   = ~(e2p0[9] | (e2p0[8:0] == 9'h00)),
   unfnp1   = ~(e2p1[9] | (e2p1[8:0] == 9'h00)),
   ezm1     = (~e2m1[9] & (e2m1[8:0] == 9'h00)),
   ezp0     = (~e2p0[9] & (e2p0[8:0] == 9'h00)),
   mskm1[3] = ~mrz & ~ovfunfm1,
   mskm1[2] = ovfm1,
   mskm1[1] = ovfm1 &  infsel,
   mskm1[0] = ovfm1 & ~infsel,
   mskp0[3] = ~mrz & ~ovfunfp0,
   mskp0[2] = ovfp0,
   mskp0[1] = ovfp0 &  infsel,
   mskp0[0] = ovfp0 & ~infsel,
   mskp1[3] = ~mrz & ~ovfunfp1,
   mskp1[2] = ovfp1,
   mskp1[1] = ovfp1 &  infsel,
   mskp1[0] = ovfp1 & ~infsel;
endmodule
module fpcry7 (co, a, b);
output       co;
input  [6:0] a;
input  [6:0] b;
wire g0n;
wire g1n;
wire g2n;
wire g3n;
wire g4n;
wire g5n;
wire g6n;
wire p1n;
wire p2n;
wire p3n;
wire p4n;
wire p5n;
wire p6n;
wire g00;
wire g21;
wire g43;
wire g65;
wire p21;
wire p43;
wire p65;
wire g20n;
wire g63n;
wire p63n;
assign g0n = ~(a[0] & b[0]);
assign g1n = ~(a[1] & b[1]);
assign g2n = ~(a[2] & b[2]);
assign g3n = ~(a[3] & b[3]);
assign g4n = ~(a[4] & b[4]);
assign g5n = ~(a[5] & b[5]);
assign g6n = ~(a[6] & b[6]);
assign p1n = ~(a[1] | b[1]);
assign p2n = ~(a[2] | b[2]);
assign p3n = ~(a[3] | b[3]);
assign p4n = ~(a[4] | b[4]);
assign p5n = ~(a[5] | b[5]);
assign p6n = ~(a[6] | b[6]);
assign g00  = ~(g0n);
assign g21  = ~(g2n & (p2n | g1n));
assign g43  = ~(g4n & (p4n | g3n));
assign g65  = ~(g6n & (p6n | g5n));
assign p21  = ~(p2n | p1n);
assign p43  = ~(p4n | p3n);
assign p65  = ~(p6n | p5n);
assign g20n = ~(g21 | (p21 & g00));
assign g63n = ~(g65 | (p65 & g43));
assign p63n = ~(p65 & p43);
assign co   = ~(g63n & (p63n | g20n));
endmodule
module fpadd8o (co, sum, a, b);
output       co;
output [7:0] sum;
input  [7:0] a;
input  [7:0] b;
wire [7:0] g0;
wire [7:0] g1;
wire [7:0] g2;
wire [6:0] g3;
wire [7:0] p0;
wire [7:2] p1;
wire [7:4] p2;
wire [7:0] s;
assign g0[0] = ~(a[0] & b[0]);
assign g0[1] = ~(a[1] & b[1]);
assign g0[2] = ~(a[2] & b[2]);
assign g0[3] = ~(a[3] & b[3]);
assign g0[4] = ~(a[4] & b[4]);
assign g0[5] = ~(a[5] & b[5]);
assign g0[6] = ~(a[6] & b[6]);
assign g0[7] = ~(a[7] & b[7]);
assign p0[0] = ~(a[0] | b[0]);
assign p0[1] = ~(a[1] | b[1]);
assign p0[2] = ~(a[2] | b[2]);
assign p0[3] = ~(a[3] | b[3]);
assign p0[4] = ~(a[4] | b[4]);
assign p0[5] = ~(a[5] | b[5]);
assign p0[6] = ~(a[6] | b[6]);
assign p0[7] = ~(a[7] | b[7]);
assign s[0]  =  (~p0[0] & g0[0]);
assign s[1]  =  (~p0[1] & g0[1]);
assign s[2]  =  (~p0[2] & g0[2]);
assign s[3]  =  (~p0[3] & g0[3]);
assign s[4]  =  (~p0[4] & g0[4]);
assign s[5]  =  (~p0[5] & g0[5]);
assign s[6]  =  (~p0[6] & g0[6]);
assign s[7]  =  (~p0[7] & g0[7]);
assign g1[0] = ~(g0[0]);
assign g1[1] = ~(g0[1] & (p0[1] | g0[0]));
assign g1[2] = ~(g0[2] & (p0[2] | g0[1]));
assign g1[3] = ~(g0[3] & (p0[3] | g0[2]));
assign g1[4] = ~(g0[4] & (p0[4] | g0[3]));
assign g1[5] = ~(g0[5] & (p0[5] | g0[4]));
assign g1[6] = ~(g0[6] & (p0[6] | g0[5]));
assign g1[7] = ~(g0[7] & (p0[7] | g0[6]));
assign p1[2] = ~(p0[2] | p0[1]);
assign p1[3] = ~(p0[3] | p0[2]);
assign p1[4] = ~(p0[4] | p0[3]);
assign p1[5] = ~(p0[5] | p0[4]);
assign p1[6] = ~(p0[6] | p0[5]);
assign p1[7] = ~(p0[7] | p0[6]);
assign g2[0] = ~(g1[0]);
assign g2[1] = ~(g1[1]);
assign g2[2] = ~(g1[2] | (p1[2] & g1[0]));
assign g2[3] = ~(g1[3] | (p1[3] & g1[1]));
assign g2[4] = ~(g1[4] | (p1[4] & g1[2]));
assign g2[5] = ~(g1[5] | (p1[5] & g1[3]));
assign g2[6] = ~(g1[6] | (p1[6] & g1[4]));
assign g2[7] = ~(g1[7] | (p1[7] & g1[5]));
assign p2[4] = ~(p1[4] & p1[2]);
assign p2[5] = ~(p1[5] & p1[3]);
assign p2[6] = ~(p1[6] & p1[4]);
assign p2[7] = ~(p1[7] & p1[5]);
assign g3[0] = ~(g2[0]);
assign g3[1] = ~(g2[1]);
assign g3[2] = ~(g2[2]);
assign g3[3] = ~(g2[3]);
assign g3[4] = ~(g2[4] & (p2[4] | g2[0]));
assign g3[5] = ~(g2[5] & (p2[5] | g2[1]));
assign g3[6] = ~(g2[6] & (p2[6] | g2[2]));
assign co    = ~(g2[7] & (p2[7] | g2[3]));
assign sum[0] =   s[0];
assign sum[1] =  (s[1] ^ g3[0]);
assign sum[2] =  (s[2] ^ g3[1]);
assign sum[3] =  (s[3] ^ g3[2]);
assign sum[4] =  (s[4] ^ g3[3]);
assign sum[5] =  (s[5] ^ g3[4]);
assign sum[6] =  (s[6] ^ g3[5]);
assign sum[7] =  (s[7] ^ g3[6]);
endmodule
