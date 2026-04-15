//===============================================================================================================================
//
//    Copyright � 2021..2025 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to Authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//    Project Name         : Cognitum
//
//    Description          : 256 processor array
//
//===============================================================================================================================
//    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//
//    NOTES:
//    All VSS pads are downbonded to the heatsink slug.   The slug is labeled pin 89
//
//===============================================================================================================================

module Cognitum_die (
`ifdef SIMULATE_CLOCK_RESET //JLSW
   input wire clockd, //***
   input wire clocki, //***
   input wire clocki_fast, //***
   input wire reset, //*** Active High Reset
   output wire [32:0]   imiso,                        // I/O interface to A2S //JLPL
   input wire  [47:0]   imosi,                        // I/O interface from A2S //JLPL
   output wire intp_news, //News Interrupt //JLPL
   input wire tz_clk,     //TileZero Raceway Input Clock //JLPL
   input wire [97:0] tz_data, //TileZero Raceway Input Data //JLPL
   output wire tz_fb,      //TileZero Raceway Input Data Ready //JLPL
   output wire rz_clk,     //JLPL
   output wire [ 97:0]  rz_data,          // Raceway to TileZero xmosi //JLPL
   input  wire rz_fb, //JLPL
`endif //JLSW - endif SIMULATE_CLOCK_RESET
// <----------- WEST -------------> <----------- SOUTH ------------> <----------- EAST ------------>   <----------- NORTH ------------>
   input  wire pad001_pin01_gpio_0, input  wire pad043_pin23_gpio_4, input  wire pad085_pin45_gpio_8,  input  wire pad127_pin67_gpio_12,
//   input  wire pad002_pin89_vss,    input  wire pad044_pin89_vss,    input  wire pad086_pin89_vss,     input  wire pad128_pin89_vss,
   input  wire pad003_pin02_vdd08,  input  wire pad045_pin24_vdd08,  input  wire pad087_pin46_vdd08,   input  wire pad129_pin68_vdd08,
//   input  wire pad004_pin89_vss,    input  wire pad046_pin89_vss,    input  wire pad088_pin89_vss,     input  wire pad130_pin89_vss,
   input  wire pad005_pin03_gpio_1, input  wire pad047_pin25_gpio_5, input  wire pad089_pin47_gpio_9,  input  wire pad131_pin69_gpio_13,
//   input  wire pad006_pin04_vdd08,  input  wire pad048_pin26_vdd08,  input  wire pad090_pin48_vdd08,   input  wire pad132_pin70_vdd08,
//   input  wire pad007_pin89_vss,    input  wire pad049_pin89_vss,    input  wire pad091_pin89_vss,     input  wire pad133_pin89_vss,
//   input  wire pad008_pin05_vdd18,  input  wire pad050_pin27_vdd18,  input  wire pad092_pin49_vdd18,   input  wire pad134_pin71_vdd18,
   input  wire pad009_pin06_WRXDp,  input  wire pad051_pin28_SRXDp,  input  wire pad093_pin50_ERXDp,   input  wire pad135_pin72_NRXDp,
   input  wire pad010_pin07_WRXDn,  input  wire pad052_pin29_SRXDn,  input  wire pad094_pin51_ERXDn,   input  wire pad136_pin73_NRXDn,
//   input  wire pad011_pin89_vss,    input  wire pad053_pin89_vss,    input  wire pad095_pin89_vss,     input  wire pad137_pin89_vss,
//   input  wire pad012_pin08_vdd18,  input  wire pad054_pin30_vdd18,  input  wire pad096_pin52_vdd18,   input  wire pad138_pin74_vdd18,
//   input  wire pad013_pin89_vss,    input  wire pad055_pin89_vss,    input  wire pad097_pin89_vss,     input  wire pad139_pin89_vss,
//   input  wire pad014_pin08_vdd18,  input  wire pad056_pin30_vdd18,  input  wire pad098_pin52_vdd18,   input  wire pad140_pin74_vdd18,
//   input  wire pad015_pin89_vss,    input  wire pad057_pin89_vss,    input  wire pad099_pin89_vss,     input  wire pad141_pin89_vss,
   input  wire pad016_pin09_WRXCp,  input  wire pad058_pin31_SRXCp,  input  wire pad100_pin53_ERXCp,   input  wire pad142_pin75_NRXCp,
   input  wire pad017_pin10_WRXCn,  input  wire pad059_pin32_SRXCn,  input  wire pad101_pin54_ERXCn,   input  wire pad143_pin76_NRXCn,
//   input  wire pad018_pin89_vss,    input  wire pad060_pin89_vss,    input  wire pad102_pin89_vss,     input  wire pad144_pin89_vss,
//   input  wire pad019_pin11_vdd08,  input  wire pad061_pin33_vdd08,  input  wire pad103_pin55_vdd08,   input  wire pad145_pin77_vdd08,
//   input  wire pad020_pin12_vdd08,  input  wire pad062_pin34_vdd08,  input  wire pad104_pin56_vdd08,   input  wire pad146_jtag_tck,
//   input  wire pad021_pin89_vss,    input  wire pad063_pin89_vss,    input  wire pad105_pin89_vss,     output wire pad147_jtag_tdo,
                                                                                                       input  wire pad146_jtag_tck,     
                                                                                                       output wire pad147_jtag_tdo,
   output wire pad022_pin13_WTXCn,  output wire pad064_pin35_STXCn,  output wire pad106_pin57_ETXCn,   input  wire pad148_jtag_tdi,
   output wire pad023_pin13_WTXCn,  output wire pad065_pin35_STXCn,  output wire pad107_pin57_ETXCn,   input  wire pad150_jtag_tms,
   output wire pad024_pin14_WTXCp,  output wire pad066_pin36_STXCp,  output wire pad108_pin58_ETXCp,   input  wire pad152_jtag_trst,
   output wire pad025_pin14_WTXCp,  output wire pad067_pin36_STXCp,  output wire pad109_pin58_ETXCp,   //input  wire pad153_pin78_vdd08,
//   input  wire pad026_pin89_vss,    input  wire pad068_pin89_vss,    input  wire pad110_pin89_vss,     input  wire pad154_pin89_vss,
//   input  wire pad027_pin15_vdd18,  input  wire pad069_pin37_vdd18,  input  wire pad111_pin59_vdd18,  
                                                                                                       output wire pad155_pin79_NTXCn,
//   input  wire pad028_pin89_vss,    input  wire pad070_pin89_vss,    input  wire pad112_pin89_vss,     
                                                                                                       output wire pad156_pin79_NTXCn,
//   input  wire pad029_pin15_vdd18,  input  wire pad071_pin37_vdd18,  input  wire pad113_pin59_vdd18,   
                                                                                                       output wire pad157_pin80_NTXCp,
//   input  wire pad030_pin89_vss,    input  wire pad072_pin89_vss,    input  wire pad114_pin89_vss,     
                                                                                                       output wire pad158_pin80_NTXCp,
   output wire pad031_pin16_WTXDn,  output wire pad073_pin38_STXDn,  output wire pad115_pin60_ETXDn,   //input  wire pad159_pin89_vss,
   output wire pad032_pin16_WTXDn,  output wire pad074_pin38_STXDn,  output wire pad116_pin60_ETXDn,   //input  wire pad160_pin81_vdd18,
   output wire pad033_pin17_WTXDp,  output wire pad075_pin39_STXDp,  output wire pad117_pin61_ETXDp,   //input  wire pad161_pin89_vss,
   output wire pad034_pin17_WTXDp,  output wire pad076_pin39_STXDp,  output wire pad118_pin61_ETXDp,   //input  wire pad162_pin81_vdd18,
//   input  wire pad035_pin18_vdd18,  input  wire pad077_pin40_vdd18,  input  wire pad119_pin62_vdd18,   input  wire pad163_pin89_vss,
//   input  wire pad036_pin89_vss,    input  wire pad078_pin89_vss,    input  wire pad120_pin89_vss,     
                                                                                                       output wire pad164_pin82_NTXDn,
//   input  wire pad037_pin19_vdd08,  input  wire pad079_pin41_vdd08,  input  wire pad121_pin63_vdd08,  
                                                                                                       output wire pad165_pin82_NTXDn,
   input  wire porx,                input  wire pad080_pin42_gpio_6, input  wire porb,                 output wire pad166_pin83_NTXDp, //JL
//   input  wire pad039_pin89_vss,    input  wire pad081_pin89_vss,    input  wire pad123_pin89_vss,    
                                                                                                       output wire pad167_pin83_NTXDp,
//   input  wire pad040_pin21_vdd08,  input  wire pad082_pin43_vdd08,  input  wire pad124_pin65_vdd08,   input  wire pad168_pin84_vdd18,
//   input  wire pad041_pin89_vss,    input  wire pad083_pin89_vss,    input  wire pad125_pin89_vss,     input  wire pad169_pin89_vss,
   input  wire pad042_pin22_gpio_3, input  wire pad084_pin44_gpio_7, input  wire pad126_pin66_gpio_11, //input  wire pad170_pin85_vdd08,
                                                                                                       input  wire pad171_pin86_gpio_14,
                                                                                                       //input  wire pad172_pin89_vss,
                                                                                                       //input  wire pad173_pin87_vdd08,
                                                                                                       //input  wire pad174_pin89_vss,
                                                                                                       input  wire pad175_pin88_gpio_15, //JL
   input VSS, input VDD_08,input VDD_18 //JL	
   );

// Local Nets -------------------------------------------------------------------------------------------------------------------

//JL - gpio wiring
   wire gpio_0_di;
   wire gpio_4_di;
   wire gpio_8_di;
   wire gpio_12_di;
   wire gpio_0_do;
   wire gpio_4_do;
   wire gpio_8_do;
   wire gpio_12_do;
   wire gpio_0_oe;
   wire gpio_4_oe;
   wire gpio_8_oe;
   wire gpio_12_oe;

   wire gpio_1_di;
   wire gpio_5_di;
   wire gpio_9_di;
   wire gpio_13_di;
   wire gpio_1_do;
   wire gpio_5_do;
   wire gpio_9_do;
   wire gpio_13_do;
   wire gpio_1_oe;
   wire gpio_5_oe;
   wire gpio_9_oe;
   wire gpio_13_oe;

   wire gpio_2_di;
   wire gpio_6_di;
   wire gpio_10_di;
   wire gpio_14_di;
   wire gpio_2_do;
   wire gpio_6_do;
   wire gpio_10_do;
   wire gpio_14_do;
   wire gpio_2_oe;
   wire gpio_6_oe;
   wire gpio_10_oe;
   wire gpio_14_oe;

   wire gpio_3_di;
   wire gpio_7_di;
   wire gpio_11_di;
   wire gpio_15_di;
   wire gpio_3_do;
   wire gpio_7_do;
   wire gpio_11_do;
   wire gpio_15_do;
   wire gpio_3_oe;
   wire gpio_7_oe;
   wire gpio_11_oe;
   wire gpio_15_oe;
//End JL

wire  WRXD_se,  SRXD_se,  ERXD_se,  NRXD_se,    //JL
      WRXC_se,  SRXC_se,  ERXC_se,  NRXC_se,    //+++
      WRXC_seb, SRXC_seb, ERXC_seb, NRXC_seb,   //+++***
      WTXC_se,  STXC_se,  ETXC_se,  NTXC_se,    //+++
      WTXD_se,  STXD_se,  ETXD_se,  NTXD_se,    //+++
      gpio_2,   gpio_6,   gpio_10,  gpio_14,
      gpio_3,   gpio_7,   gpio_11,  gpio_15;

//AFE Configurations
wire [1:0] Wtx_slew,          Stx_slew,         Etx_slew,         Ntx_slew;   //*** New connection wires
wire       Wpmu_pd,           Spmu_pd,          Epmu_pd,          Npmu_pd;    //*** New connection wires
wire       Wrx_pd,            Srx_pd,           Erx_pd,           Nrx_pd;     //*** New connection wires
wire       Wtx_pd,            Stx_pd,           Etx_pd,           Ntx_pd;     //*** New connection wires
wire       sig_por;                                                           //*** New connection wire

wire [4:0] Wrx_cdac_p,        Srx_cdac_p,       Erx_cdac_p,       Nrx_cdac_p;
wire [4:0] Wrx_cdac_m,        Srx_cdac_m,       Erx_cdac_m,       Nrx_cdac_m; //JL
wire [3:0] Wrx_bias_trim,     Srx_bias_trim,    Erx_bias_trim,    Nrx_bias_trim;
wire [5:0] Wrx_rterm_trim,    Srx_rterm_trim,   Erx_rterm_trim,   Nrx_rterm_trim;
wire [3:0] Wtx_offset_trim,   Stx_offset_trim,  Etx_offset_trim,  Ntx_offset_trim;
wire [5:0] Wpmu_bias_trim,    Spmu_bias_trim,   Epmu_bias_trim,   Npmu_bias_trim;

wire Wpower_down,Spower_down,Epower_down,Npower_down; //JL
wire [4:0] Wanalog_test, Sanalog_test, Eanalog_test, Nanalog_test; //JL
wire [3:0] Wrx_gain_trim, Srx_gain_trim, Erx_gain_trim, Nrx_gain_trim; //JL**
wire [3:0] Wrx_offset_trim, Srx_offset_trim, Erx_offset_trim, Nrx_offset_trim; //JL**
wire [3:0] Wtx_bias_trim, Stx_bias_trim, Etx_bias_trim, Ntx_bias_trim; //JL

wire  [5:0] Wlvds_spare_in,      Slvds_spare_in,      Elvds_spare_in,      Nlvds_spare_in; //JLSW
wire  [7:0] Wlvds_spare_out,     Slvds_spare_out,     Elvds_spare_out,     Nlvds_spare_out; //JLSW

wire sync_phase; //JLPL - 10_23_25

// Instancess -------------------------------------------------------------------------------------------------------------------

Cognitum_pads i_PADS (
   .pad_gpio_0(pad001_pin01_gpio_0),  .pad_gpio_4(pad043_pin23_gpio_4),  .pad_gpio_8 (pad085_pin45_gpio_8),   .pad_gpio_12 (pad127_pin67_gpio_12),
   .pad_gpio_1(pad005_pin03_gpio_1),  .pad_gpio_5(pad047_pin25_gpio_5),  .pad_gpio_9 (pad089_pin47_gpio_9),   .pad_gpio_13 (pad131_pin69_gpio_13),
   .pad_WRXDp (pad009_pin06_WRXDp),   .pad_SRXDp (pad051_pin28_SRXDp),   .pad_ERXDp  (pad093_pin50_ERXDp),    .pad_NRXDp   (pad135_pin72_NRXDp),
   .pad_WRXDn (pad010_pin07_WRXDn),   .pad_SRXDn (pad052_pin29_SRXDn),   .pad_ERXDn  (pad094_pin51_ERXDn),    .pad_NRXDn   (pad136_pin73_NRXDn),
   .pad_WRXCp (pad016_pin09_WRXCp),   .pad_SRXCp (pad058_pin31_SRXCp),   .pad_ERXCp  (pad100_pin53_ERXCp),    .pad_NRXCp   (pad142_pin75_NRXCp),
   .pad_WRXCn (pad017_pin10_WRXCn),   .pad_SRXCn (pad059_pin32_SRXCn),   .pad_ERXCn  (pad101_pin54_ERXCn),    .pad_NRXCn   (pad143_pin76_NRXCn),
   .pad_WTXCna(pad022_pin13_WTXCn),   .pad_STXCna(pad064_pin35_STXCn),   .pad_ETXCna (pad106_pin57_ETXCn),    .pad_jtag_tck(pad146_jtag_tck),
   .pad_WTXCnb(pad023_pin13_WTXCn),   .pad_STXCnb(pad065_pin35_STXCn),   .pad_ETXCnb (pad107_pin57_ETXCn),    .pad_jtag_tdo(pad147_jtag_tdo),
   .pad_WTXCpa(pad024_pin14_WTXCp),   .pad_STXCpa(pad066_pin36_STXCp),   .pad_ETXCpa (pad108_pin58_ETXCp),    .pad_jtag_tdi(pad148_jtag_tdi), //JL
   .pad_WTXCpb(pad025_pin14_WTXCp),   .pad_STXCpb(pad067_pin36_STXCp),   .pad_ETXCpb (pad109_pin58_ETXCp),    .pad_jtag_tms(pad150_jtag_tms),
   .pad_WTXDna(pad031_pin16_WTXDn),   .pad_STXDna(pad073_pin38_STXDn),   .pad_ETXDna (pad115_pin60_ETXDn),    .pad_jtag_trst(pad152_jtag_trst), //JL
   .pad_WTXDnb(pad032_pin16_WTXDn),   .pad_STXDnb(pad074_pin38_STXDn),   .pad_ETXDnb (pad116_pin60_ETXDn),    .pad_NTXCna  (pad155_pin79_NTXCn),
   .pad_WTXDpa(pad033_pin17_WTXDp),   .pad_STXDpa(pad075_pin39_STXDp),   .pad_ETXDpa (pad117_pin61_ETXDp),    .pad_NTXCnb  (pad156_pin79_NTXCn),
   .pad_WTXDpb(pad034_pin17_WTXDp),   .pad_STXDpb(pad076_pin39_STXDp),   .pad_ETXDpb (pad118_pin61_ETXDp),    .pad_NTXCpa  (pad157_pin80_NTXCp), 
                                                                                                              .pad_NTXCpb  (pad158_pin80_NTXCp), //JL
   .pad_gpio_2(porx),                 .pad_gpio_6(pad080_pin42_gpio_6),  .pad_gpio_10(porb),                  .pad_NTXDna  (pad164_pin82_NTXDn), //JL
   .pad_gpio_3(pad042_pin22_gpio_3),  .pad_gpio_7(pad084_pin44_gpio_7),  .pad_gpio_11(pad126_pin66_gpio_11),  .pad_NTXDnb  (pad165_pin82_NTXDn),
                                                                                                              .pad_NTXDpa  (pad166_pin83_NTXDp),
                                                                                                              .pad_NTXDpb  (pad167_pin83_NTXDp),
                                                                                                              .pad_gpio_14 (pad171_pin86_gpio_14),
                                                                                                              .pad_gpio_15 (pad175_pin88_gpio_15),

   .sig_gpio_0_di(gpio_0_di),         .sig_gpio_4_di(gpio_4_di),         .sig_gpio_8_di (gpio_8_di ),         .sig_gpio_12_di(gpio_12_di),
   .sig_gpio_0_do(gpio_0_do),         .sig_gpio_4_do(gpio_4_do),         .sig_gpio_8_do (gpio_8_do ),         .sig_gpio_12_do(gpio_12_do),
   .sig_gpio_0_oe(gpio_0_oe),         .sig_gpio_4_oe(gpio_4_oe),         .sig_gpio_8_oe (gpio_8_oe ),         .sig_gpio_12_oe(gpio_12_oe),
   .sig_gpio_1_di(gpio_1_di),         .sig_gpio_5_di(gpio_5_di),         .sig_gpio_9_di (gpio_9_di ),         .sig_gpio_13_di(gpio_13_di),
   .sig_gpio_1_do(gpio_1_do),         .sig_gpio_5_do(gpio_5_do),         .sig_gpio_9_do (gpio_9_do ),         .sig_gpio_13_do(gpio_13_do),
   .sig_gpio_1_oe(gpio_1_oe),         .sig_gpio_5_oe(gpio_5_oe),         .sig_gpio_9_oe (gpio_9_oe ),         .sig_gpio_13_oe(gpio_13_oe),
   .sig_WRXD_se  (WRXD_se),           .sig_SRXD_se  (SRXD_se),           .sig_ERXD_se   (ERXD_se),            .sig_NRXD_se   (NRXD_se), //JL
   .sig_WRXC_se  (WRXC_se),           .sig_SRXC_se  (SRXC_se),           .sig_ERXC_se   (ERXC_se),            .sig_NRXC_se   (NRXC_se), //JL
   .sig_WRXC_seb (WRXC_seb),          .sig_SRXC_seb (SRXC_seb),          .sig_ERXC_seb  (ERXC_seb),           .sig_NRXC_seb  (NRXC_seb), //JL
   .sig_WTXC_se  (WTXC_se),           .sig_STXC_se  (STXC_se),           .sig_ETXC_se   (ETXC_se),            .sig_jtag_tck  (tck ), //JL
   .sig_WTXD_se  (WTXD_se),           .sig_STXD_se  (STXD_se),           .sig_ETXD_se   (ETXD_se),            .sig_jtag_tdo  (tdo ),
   .sig_gpio_2_di(gpio_2_di),         .sig_gpio_6_di(gpio_6_di),         .sig_gpio_10_di(gpio_10_di),         .sig_jtag_tdi  (tdi ),
   .sig_gpio_2_do(gpio_2_do),         .sig_gpio_6_do(gpio_6_do),         .sig_gpio_10_do(gpio_10_do),         .sig_jtag_tms  (tms ),
                                      .sig_gpio_6_oe(gpio_6_oe),                                              .sig_jtag_trst (trst), //JL
   .sig_gpio_3_di(gpio_3_di),         .sig_gpio_7_di(gpio_7_di),         .sig_gpio_11_di(gpio_11_di),         .sig_NTXC_se   (NTXC_se), //JL
   .sig_gpio_3_do(gpio_3_do),         .sig_gpio_7_do(gpio_7_do),         .sig_gpio_11_do(gpio_11_do),         .sig_NTXD_se   (NTXD_se), //JL
   .sig_gpio_3_oe(gpio_3_oe),         .sig_gpio_7_oe(gpio_7_oe),         .sig_gpio_11_oe(gpio_11_oe),         .sig_gpio_14_di(gpio_14_di),
                                                                                                              .sig_gpio_14_do(gpio_14_do),
                                                                                                              .sig_gpio_14_oe(gpio_14_oe),
                                                                                                              .sig_gpio_15_di(gpio_15_di),
                                                                                                              .sig_gpio_15_do(gpio_15_do),
                                                                                                              .sig_gpio_15_oe(gpio_15_oe),

// .Wrx_offset_trim     (Wrx_offset_trim),         .Srx_offset_trim     (Srx_offset_trim),   //*** - Remove signals
// .Wrx_gain_trim       (Wrx_gain_trim),           .Srx_gain_trim       (Srx_gain_trim),     //*** - Remove signals
// .Wtx_bias_trim       (Wtx_bias_trim),           .Stx_bias_trim       (Stx_bias_trim),     //*** - Remove signals
   .rx_rterm_trim_W     (Wrx_rterm_trim),          .rx_rterm_trim_S     (Srx_rterm_trim),    //*** - Update Module signal name
   .tx_slew_W           (Wtx_slew),                .tx_slew_S           (Stx_slew),          //*** - Add new signals to Cognitum_Pads this level
   .rx_cdac_p_W         (Wrx_cdac_p),              .rx_cdac_p_S         (Srx_cdac_p),        //*** - Update Module signal name
   .rx_cdac_m_W         (Wrx_cdac_m),              .rx_cdac_m_S         (Srx_cdac_m),        //*** - Update Module signal name
   .rx_bias_trim_W      (Wrx_bias_trim),           .rx_bias_trim_S      (Srx_bias_trim),     //*** - Update Module signal name
   .tx_offset_trim_W    (Wtx_offset_trim),         .tx_offset_trim_S    (Stx_offset_trim),   //*** - Update Module signal name
   .pmu_bias_trim_W     (Wpmu_bias_trim),          .pmu_bias_trim_S     (Spmu_bias_trim),    //*** - Update Module signal name
   .pmu_pd_W            (Wpower_down),             .pmu_pd_S            (Spower_down),           //*** Add new signals to Cognitum_Pads this level //***JL
   .rx_pd_W             (Wpower_down),             .rx_pd_S             (Spower_down),            //*** Add new signals to Cognitum_Pads this level //***JL
   .tx_pd_W             (Wpower_down),             .tx_pd_S             (Spower_down),            //*** Add new signals to Cognitum_Pads this level //***JL
   .sig_por             (sig_por),                                                           //*** Add new signal to Cognitum_Pads this level

// .Erx_offset_trim     (Erx_offset_trim),         .Nrx_offset_trim     (Nrx_offset_trim),   //*** - Remove signals
// .Erx_gain_trim       (Erx_gain_trim),           .Nrx_gain_trim       (Nrx_gain_trim),     //*** - Remove signals
// .Etx_bias_trim       (Etx_bias_trim),           .Ntx_bias_trim       (Ntx_bias_trim),     //*** - Remove signals
   .rx_rterm_trim_E     (Erx_rterm_trim),          .rx_rterm_trim_N     (Nrx_rterm_trim),    //*** - Update Module signal name
   .tx_slew_E           (Etx_slew),                .tx_slew_N           (Ntx_slew),          //*** - Add new signals to Cognitum_Pads this level
   .rx_cdac_p_E         (Erx_cdac_p),              .rx_cdac_p_N         (Nrx_cdac_p),        //*** - Update Module signal name
   .rx_cdac_m_E         (Erx_cdac_m),              .rx_cdac_m_N         (Nrx_cdac_m),        //*** - Update Module signal name
   .rx_bias_trim_E      (Erx_bias_trim),           .rx_bias_trim_N      (Nrx_bias_trim),     //*** - Update Module signal name
   .tx_offset_trim_E    (Etx_offset_trim),         .tx_offset_trim_N    (Ntx_offset_trim),   //*** - Update Module signal name
   .pmu_bias_trim_E     (Epmu_bias_trim),          .pmu_bias_trim_N     (Npmu_bias_trim),    //*** - Update Module signal name
   .pmu_pd_E            (Epower_down),             .pmu_pd_N            (Npower_down),           //*** Add new signals to Cognitum_Pads this level //***JL
   .rx_pd_E             (Epower_down),             .rx_pd_N             (Npower_down),            //*** Add new signals to Cognitum_Pads this level //***JL
   .tx_pd_E             (Epower_down),             .tx_pd_N             (Npower_down),            //*** Add new signals to Cognitum_Pads this level //***JL

   .Wanalog_test (Wanalog_test), .Sanalog_test (Sanalog_test), .Eanalog_test (Eanalog_test), .Nanalog_test (Nanalog_test), //JL
   .vddcore (VDD_08), .vddio (VDD_18), .vss (VSS) //JL
   );

//wire PoR_bypass = gpio_2_do; //JLSW
//wire PoR_extern = gpio_10_do; //JLSW 
wire PoR_bypass = 1'b0; //JLSW - temporary
wire PoR_extern = 1'b0; //JLSW - temporary 

Cognitum_core i_CORE (
`ifdef SIMULATE_CLOCK_RESET //JLSW
   .clockd(clockd),              //JLSW
   .clocki(clocki),              //JLSW
   .clocki_fast(clocki_fast),    //JLSW
   .reset(reset),                //JLSW
   .imiso(imiso),                //JLPL
   .imosi(imosi),                //JLPL
   .intp_news(intp_news),        //JLPL
   .tz_clk(tz_clk),              //JLPL
   .tz_data(tz_data),            //JLPL
   .tz_fb(tz_fb),                //JLPL
   .rz_clk(rz_clk),              //JLPL
   .rz_data(rz_data),            //JLPL
   .rz_fb(rz_fb),                //JLPL
`endif //JLSW - endif SIMULATE_CLOCK_RESET
   .gpio_0_di(gpio_0_di),        .gpio_4_di(gpio_4_di),        .gpio_8_di (gpio_8_di ),        .gpio_12_di(gpio_12_di),
   .gpio_0_do(gpio_0_do),        .gpio_4_do(gpio_4_do),        .gpio_8_do (gpio_8_do ),        .gpio_12_do(gpio_12_do),
   .gpio_0_oe(gpio_0_oe),        .gpio_4_oe(gpio_4_oe),        .gpio_8_oe (gpio_8_oe ),        .gpio_12_oe(gpio_12_oe),
   .gpio_1_di(gpio_1_di),        .gpio_5_di(gpio_5_di),        .gpio_9_di (gpio_9_di ),        .gpio_13_di(gpio_13_di),
   .gpio_1_do(gpio_1_do),        .gpio_5_do(gpio_5_do),        .gpio_9_do (gpio_9_do ),        .gpio_13_do(gpio_13_do),
   .gpio_1_oe(gpio_1_oe),        .gpio_5_oe(gpio_5_oe),        .gpio_9_oe (gpio_9_oe ),        .gpio_13_oe(gpio_13_oe),
   .WRXD_se  (WRXD_se),          .SRXD_se (SRXD_se),           .ERXD_se   (ERXD_se),           .NRXD_se   (NRXD_se), //+++
   .WRXC_se  (WRXC_se),          .SRXC_se (SRXC_se),           .ERXC_se   (ERXC_se),           .NRXC_se   (NRXC_se), //+++
   .WRXC_seb (WRXC_seb),         .SRXC_seb (SRXC_seb),         .ERXC_seb  (ERXC_seb),          .NRXC_seb  (NRXC_seb), //+++***
   .WTXC_se  (WTXC_se),          .STXC_se (STXC_se),           .ETXC_se   (ETXC_se),           .tck       (tck), //+++
   .WTXD_se  (WTXD_se),          .STXD_se (STXD_se),           .ETXD_se   (ETXD_se),           .tdo       (tdo), //***
   .gpio_2_di(gpio_2_di),        .gpio_6_di(gpio_6_di),        .gpio_10_di(gpio_10_di),        .tdi       (tdi),
   .gpio_2_do(gpio_2_do),        .gpio_6_do(gpio_6_do),        .gpio_10_do(gpio_10_do),        .tms       (tms),
   .gpio_2_oe(gpio_2_oe),        .gpio_6_oe(gpio_6_oe),        .gpio_10_oe(gpio_10_oe),        .trst      (trst),
   .gpio_3_di(gpio_3_di),        .gpio_7_di(gpio_7_di),        .gpio_11_di(gpio_11_di),        .NTXC_se   (NTXC_se), //***
   .gpio_3_do(gpio_3_do),        .gpio_7_do(gpio_7_do),        .gpio_11_do(gpio_11_do),        .NTXD_se   (NTXD_se), //***
   .gpio_3_oe(gpio_3_oe),        .gpio_7_oe(gpio_7_oe),        .gpio_11_oe(gpio_11_oe),        .gpio_14_di(gpio_14_di),
                                                                                               .gpio_14_do(gpio_14_do),
                                                                                               .gpio_14_oe(gpio_14_oe),
                                                                                               .gpio_15_di(gpio_15_di),
                                                                                               .gpio_15_do(gpio_15_do),
                                                                                               .gpio_15_oe(gpio_15_oe),
   .Wrx_rterm_trim      (Wrx_rterm_trim),          .Srx_rterm_trim      (Srx_rterm_trim),
   .Wrx_offset_trim     (Wrx_offset_trim),         .Srx_offset_trim     (Srx_offset_trim),
   .Wrx_cdac_p          (Wrx_cdac_p),              .Srx_cdac_p          (Srx_cdac_p),
   .Wrx_cdac_m          (Wrx_cdac_m),              .Srx_cdac_m          (Srx_cdac_m),
   .Wrx_bias_trim       (Wrx_bias_trim),           .Srx_bias_trim       (Srx_bias_trim),
   .Wrx_gain_trim       (Wrx_gain_trim),           .Srx_gain_trim       (Srx_gain_trim),
   .Wtx_bias_trim       (Wtx_bias_trim),           .Stx_bias_trim       (Stx_bias_trim),
   .Wtx_offset_trim     (Wtx_offset_trim),         .Stx_offset_trim     (Stx_offset_trim),
   .Wpmu_bias_trim      (Wpmu_bias_trim),          .Spmu_bias_trim      (Spmu_bias_trim),
   .Wlvds_spare_in      (Wlvds_spare_in),          .Slvds_spare_in      (Slvds_spare_in), //JLSW
   .Wlvds_spare_out     (Wlvds_spare_out),         .Slvds_spare_out     (Slvds_spare_out),
   .Wanalog_test        (Wanalog_test),            .Sanalog_test        (Sanalog_test),
   .Wanalog_reserved    (Wanalog_reserved),        .Sanalog_reserved    (Sanalog_reserved),
   .Wpreserve_spare_net (Wpreserve_spare_net),     .Spreserve_spare_net (Spreserve_spare_net),
   .Wpower_down         (Wpower_down),             .Spower_down         (Spower_down),

   .Erx_rterm_trim      (Erx_rterm_trim),          .Nrx_rterm_trim      (Nrx_rterm_trim),
   .Erx_offset_trim     (Erx_offset_trim),         .Nrx_offset_trim     (Nrx_offset_trim),
   .Erx_cdac_p          (Erx_cdac_p),              .Nrx_cdac_p          (Nrx_cdac_p),
   .Erx_cdac_m          (Erx_cdac_m),              .Nrx_cdac_m          (Nrx_cdac_m),
   .Erx_bias_trim       (Erx_bias_trim),           .Nrx_bias_trim       (Nrx_bias_trim),
   .Erx_gain_trim       (Erx_gain_trim),           .Nrx_gain_trim       (Nrx_gain_trim),
   .Etx_bias_trim       (Etx_bias_trim),           .Ntx_bias_trim       (Ntx_bias_trim),
   .Etx_offset_trim     (Etx_offset_trim),         .Ntx_offset_trim     (Ntx_offset_trim),
   .Epmu_bias_trim      (Epmu_bias_trim),          .Npmu_bias_trim      (Npmu_bias_trim),
   .Elvds_spare_in      (Elvds_spare_in),          .Nlvds_spare_in      (Nlvds_spare_in),
   .Elvds_spare_out     (Elvds_spare_out),         .Nlvds_spare_out     (Nlvds_spare_out),
   .Eanalog_test        (Eanalog_test),            .Nanalog_test        (Nanalog_test),
   .Eanalog_reserved    (Eanalog_reserved),        .Nanalog_reserved    (Nanalog_reserved),
   .Epreserve_spare_net (Epreserve_spare_net),     .Npreserve_spare_net (Npreserve_spare_net), //JL
   .Epower_down         (Epower_down),             .Npower_down         (Npower_down),              //JL
   .sync_phase          (sync_phase),                                                               //JLPL - 10_23_25
   .PoR_bypass          (PoR_bypass),                                                               //JLSW
   .PoR_extern          (PoR_extern),                                                               //JLSW
   .POR                 (sig_por)                                                                  //JL
   );

endmodule

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////(//////
