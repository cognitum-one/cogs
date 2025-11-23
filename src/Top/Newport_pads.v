//===============================================================================================================================
//
//    Copyright © 2021.2025 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to Authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//    Project Name         : Newport
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
//    All VSS pads are downbonded to the heatsink slug.   The slug may therefore be considered as pin 89
//
//===============================================================================================================================

`ifdef  synthesis //JLPL
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2_project_settings.vh"
`else
   `include "A2_project_settings.vh"
`endif

//`timescale 1ps/1fs

// RKSW 06/02/2025 - Connected all RTO and rto pins to wire rto.
//                   Connected all SNS and sns pins to wire sns.
//                   Added PVSENSE cell.

// RKSW 06/17/2025 - Changed analog_test control bits to the lvds_rx_top modules, 8 places.
//                   Example for West shown below. The other directions were changed similarly.
//                   Original {Wanalog_test}
//                   Changed for West Data RX  {tielo_vss,tielo_vss,Wanalog_test[2:0]}
//                   Changed for West Clock RX {tielo_vss,Wanalog_test[3],tielo_vss,Wanalog_test[1:0]}

//////////////////////////////////////////////////////////////
///////   Attention!! Arret!! Achtung!!  /////////////////////
// The tie cells in this file were hand placed for simulation.
// The PNR tool needs to determine the proper amount.
//////////////////////////////////////////////////////////////

/* GPIO LUT
gpio_0 = WRXD
gpio_1 = WRXC
gpio_2 = PoR_bypass
gpio_3 = WTXD

gpio_4 = SRXD
gpio_5 = SRXC
gpio_6 = STXC
gpio_7 = STXD

gpio_8 = ERXD
gpio_9 = ERXC
gpio_10 = PoR_extern
gpio_11 = ETXD

gpio_12 = NRXD
gpio_13 = NRXC
gpio_14 = NTXC
gpio_15 = NTXD
*/

module Newport_pads 
  #(parameter PMU_START_DELAY = 1000, //RKSW
    parameter POR_DELAY = 500000) //JLPL - 7_24_25 Change to 500000 to get 500ns of POR
   (
   // PADS
   inout  wire  pad_gpio_0,      inout  wire pad_gpio_4,       inout  wire pad_gpio_8,       inout  wire pad_gpio_12,
   inout  wire  pad_gpio_1,      inout  wire pad_gpio_5,       inout  wire pad_gpio_9,       inout  wire pad_gpio_13,
   input  wire  pad_WRXDp,       input  wire pad_SRXDp,        input  wire pad_ERXDp,        input  wire pad_NRXDp,
   input  wire  pad_WRXDn,       input  wire pad_SRXDn,        input  wire pad_ERXDn,        input  wire pad_NRXDn,
   input  wire  pad_WRXCp,       input  wire pad_SRXCp,        input  wire pad_ERXCp,        input  wire pad_NRXCp,
   input  wire  pad_WRXCn,       input  wire pad_SRXCn,        input  wire pad_ERXCn,        input  wire pad_NRXCn,
   output wire  pad_WTXCna,      output wire pad_STXCna,       output wire pad_ETXCna,       output wire pad_NTXCna,
   output wire  pad_WTXCnb,      output wire pad_STXCnb,       output wire pad_ETXCnb,       output wire pad_NTXCnb,
   output wire  pad_WTXCpa,      output wire pad_STXCpa,       output wire pad_ETXCpa,       output wire pad_NTXCpa,
   output wire  pad_WTXCpb,      output wire pad_STXCpb,       output wire pad_ETXCpb,       output wire pad_NTXCpb,
   output wire  pad_WTXDna,      output wire pad_STXDna,       output wire pad_ETXDna,       output wire pad_NTXDna,
   output wire  pad_WTXDnb,      output wire pad_STXDnb,       output wire pad_ETXDnb,       output wire pad_NTXDnb,
   output wire  pad_WTXDpa,      output wire pad_STXDpa,       output wire pad_ETXDpa,       output wire pad_NTXDpa,
   output wire  pad_WTXDpb,      output wire pad_STXDpb,       output wire pad_ETXDpb,       output wire pad_NTXDpb,
   inout  wire  pad_gpio_2,      inout  wire pad_gpio_6,       inout  wire pad_gpio_10,      inout  wire pad_gpio_14,
   inout  wire  pad_gpio_3,      inout  wire pad_gpio_7,       inout  wire pad_gpio_11,      inout  wire pad_gpio_15,
   input  wire  pad_jtag_tck,
   output wire  pad_jtag_tdo,
   input  wire  pad_jtag_tdi,
   input  wire  pad_jtag_tms,
   input  wire  pad_jtag_trst,

   // Signals to & from the core
   input  wire  sig_gpio_0_di,   input  wire sig_gpio_4_di,    input  wire sig_gpio_8_di,    input  wire sig_gpio_12_di,
   output wire  sig_gpio_0_do,   output wire sig_gpio_4_do,    output wire sig_gpio_8_do,    output wire sig_gpio_12_do,
   input  wire  sig_gpio_0_oe,   input  wire sig_gpio_4_oe,    input  wire sig_gpio_8_oe,    input  wire sig_gpio_12_oe,
   input  wire  sig_gpio_1_di,   input  wire sig_gpio_5_di,    input  wire sig_gpio_9_di,    input  wire sig_gpio_13_di,
   output wire  sig_gpio_1_do,   output wire sig_gpio_5_do,    output wire sig_gpio_9_do,    output wire sig_gpio_13_do,
   input  wire  sig_gpio_1_oe,   input  wire sig_gpio_5_oe,    input  wire sig_gpio_9_oe,    input  wire sig_gpio_13_oe,
   output wire  sig_WRXD_se,     output wire sig_SRXD_se,      output wire sig_ERXD_se,      output wire sig_NRXD_se,
   output wire  sig_WRXC_se,     output wire sig_SRXC_se,      output wire sig_ERXC_se,      output wire sig_NRXC_se,
   output wire  sig_WRXC_seb,    output wire sig_SRXC_seb,     output wire sig_ERXC_seb,     output wire sig_NRXC_seb, //JL
   input  wire  sig_WTXC_se,     input  wire sig_STXC_se,      input  wire sig_ETXC_se,      input  wire sig_NTXC_se,
   input  wire  sig_WTXD_se,     input  wire sig_STXD_se,      input  wire sig_ETXD_se,      input  wire sig_NTXD_se,
   input  wire  sig_gpio_2_di,   input  wire sig_gpio_6_di,    input  wire sig_gpio_10_di,   input  wire sig_gpio_14_di,
   output wire  sig_gpio_2_do,   output wire sig_gpio_6_do,    output wire sig_gpio_10_do,   output wire sig_gpio_14_do,
                                 input  wire sig_gpio_6_oe,                                  input  wire sig_gpio_14_oe,
   input  wire  sig_gpio_3_di,   input  wire sig_gpio_7_di,    input  wire sig_gpio_11_di,   input  wire sig_gpio_15_di,
   output wire  sig_gpio_3_do,   output wire sig_gpio_7_do,    output wire sig_gpio_11_do,   output wire sig_gpio_15_do,
   input  wire  sig_gpio_3_oe,   input  wire sig_gpio_7_oe,    input  wire sig_gpio_11_oe,   input  wire sig_gpio_15_oe,
   output wire  sig_jtag_tck,
   input  wire  sig_jtag_tdo,
   output wire  sig_jtag_tdi,
   output wire  sig_jtag_tms,
   output wire  sig_jtag_trst,
   // AFE Configuration
   input wire [4:0] rx_cdac_p_W, rx_cdac_p_S, rx_cdac_p_E, rx_cdac_p_N,
   input wire [4:0] rx_cdac_m_W, rx_cdac_m_S, rx_cdac_m_E, rx_cdac_m_N,
   input wire [3:0] rx_bias_trim_W, rx_bias_trim_S, rx_bias_trim_E, rx_bias_trim_N,
   input wire [5:0] rx_rterm_trim_W, rx_rterm_trim_S, rx_rterm_trim_E, rx_rterm_trim_N,
   input wire [1:0] tx_slew_W, tx_slew_S, tx_slew_E, tx_slew_N,
   input wire [3:0] tx_offset_trim_W, tx_offset_trim_S, tx_offset_trim_E, tx_offset_trim_N,
   input wire [5:0] pmu_bias_trim_W, pmu_bias_trim_S, pmu_bias_trim_E, pmu_bias_trim_N,
   input wire [4:0] Wanalog_test, Sanalog_test, Eanalog_test, Nanalog_test, //***JL
   input wire pmu_pd_W, pmu_pd_S, pmu_pd_E, pmu_pd_N,
   input wire rx_pd_W, rx_pd_S, rx_pd_E, rx_pd_N,
   input wire tx_pd_W, tx_pd_S, tx_pd_E, tx_pd_N,
   output wire sig_por,

   // Power/Ground
   inout wire vddcore, vddio, vss
   
);

   /// Patch until the actual tie cells are instantiated - Start
	reg tiehi;
	wire tiehi_vddcore;
	assign tiehi_vddcore = tiehi;
	reg tielo;
	wire tielo_vss;
	assign tielo_vss = tielo;
	
	initial
	  begin
	    tiehi = 1'b1;
	    tielo = 1'b0;
	  end
   /// Patch until the actual tie cells are instantiated - End
   
	wire [1:0] ibp_50u_W, ibp_50u_S, ibp_50u_E, ibp_50u_N;
	wire [1:0] ibn_200u_W, ibn_200u_S, ibn_200u_E, ibn_200u_N;
	wire [1:0] ibp_200u_W, ibp_200u_S, ibp_200u_E, ibp_200u_N;
	wire rto, sns;
	
	//RKSW
	PVSENSE_18_18_NT_DR_V pad000_nopin_pvsense(
		.RTO(rto),
		.SNS(sns),
		.DVDD(vddio),
		.DVSS(vss),
		.VDD(vddcore),
		.VSS(vss),
		.RETOFF(tiehi_vddcore),
		.RETON(tielo_vss)
	);

	///////////////////////////
        ////////// West //////////
	///////////////////////////

	PBIDIR_18_18_NT_DR_H pad001_pin01_gpio_0_W (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                            .PO(), .Y(sig_gpio_0_do), .PAD(pad_gpio_0), .A(sig_gpio_0_di), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                                    .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_0_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVSS_08_08_NT_DR_H  pad002_pin89_vss_W   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVDD_08_08_NT_DR_H  pad003_pin02_vdd08_W (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_H  pad004_pin89_vss_W   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	PBIDIR_18_18_NT_DR_H pad005_pin03_gpio_1_W (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                            .PO(), .Y(sig_gpio_1_do), .PAD(pad_gpio_1), .A(sig_gpio_1_di), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                                    .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_1_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVDD_08_08_NT_DR_H  pad006_pin04_vdd08_W (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_H  pad007_pin89_vss_W   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_H pad008_pin05_vdd18_W (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));

	lvds_rx_top pad009_010_pin06_07_WRXD (.vddc(vddcore), .vss(vss), .vddio(vddio), .rto(rto), .sns(sns),
	                          .pd_c(rx_pd_W), .trim_c(tielo_vss), .rx_cdac_p(rx_cdac_p_W), .rx_cdac_m(rx_cdac_m_W),
	                          .rx_bias_trim(rx_bias_trim_W), .rx_rterm_trim(rx_rterm_trim_W), //JL
	                          .rx_inp(pad_WRXDp), .rx_inm(pad_WRXDn), .rx_out(sig_WRXD_se), .rx_outb(), .ibn_200u(ibn_200u_W[0]), .ibp_50u(ibp_50u_W[0]),
	                          .analog_test({tielo_vss,tielo_vss,Wanalog_test[2:0]}), .testbus_in_1(tb_rx2_1_W), .testbus_in_2(tb_rx2_2_W), .testbus_out_1(tb_rx1_1_W), .testbus_out_2(tb_rx1_2_W),
	                          .testbus_off(tb_off_W), .vref_tg_en(vref_tg_en_W), .vref_in(vref_tb_W),
	                          .ampout_p(), .ampout_m(), .compout_p(), .compout_m());

	PDVSS_18_18_NT_DR_H pad011_pin89_vss   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_H pad012_pin08_vdd18 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVSS_18_18_NT_DR_H pad013_pin89_vss   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_H pad014_pin08_vdd18 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVSS_18_18_NT_DR_H pad015_pin89_vss   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	                          
	lvds_rx_top pad016_17_pin09_10_WRXC (.vddc(vddcore), .vss(vss), .vddio(vddio), .rto(rto), .sns(sns),
	                          .pd_c(rx_pd_W), .trim_c(tielo_vss), .rx_cdac_p(rx_cdac_p_W), .rx_cdac_m(rx_cdac_m_W),
	                          .rx_bias_trim(rx_bias_trim_W), .rx_rterm_trim(rx_rterm_trim_W), //JL
	                          .rx_inp(pad_WRXCp), .rx_inm(pad_WRXCn), .rx_out(sig_WRXC_se), .rx_outb(sig_WRXC_seb), .ibn_200u(ibn_200u_W[1]), .ibp_50u(ibp_50u_W[1]),
	                          .analog_test({tielo_vss,Wanalog_test[3],tielo_vss,Wanalog_test[1:0]}), .testbus_in_1(tb_rx1_1_W), .testbus_in_2(tb_rx1_2_W), .testbus_out_1(tb_rx2_1_W), .testbus_out_2(tb_rx2_2_W),
	                          .testbus_off(), .vref_tg_en(), .vref_in(vref_tb_W),
	                          .ampout_p(), .ampout_m(), .compout_p(), .compout_m());
	
	PVSS_08_08_NT_DR_H  pad018_pin89_vss (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVDD_08_08_NT_DR_H  pad019_pin11_vdd08 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));

	ts_pmu  #(.PMU_START_DELAY(PMU_START_DELAY)) pmu_lvds_W (.vddd(vddcore), .vssio(vss), .vddio(vddio),
	                        .pd(pmu_pd_W), .pmu_bias_trim(pmu_bias_trim_W), .vref_tg_en(vref_tg_en_W), .testbus_off(tb_off_W),
				.vref(vref_W), .rref(rref_W), .vref_out(vref_tb_W), .ibp_200u(ibp_200u_W), .ibp_50u(ibp_50u_W), .ibn_200u(ibn_200u_W));
	
	PVDD_08_08_NT_DR_H  pad020_pin12_vdd08_W (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_H  pad021_pin89_vss_W   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));

	lvds_tx_top pad022_023_024_025_pin13_14_WTXC (.vddd(vddcore), .vss(vss), .vddio(vddio), .rto(rto), .sns(sns),
	                          .enb(tx_pd_W), .tx_offset_trim(tx_offset_trim_W), .tx_slew(tx_slew_W), 
	                          .tx_in(sig_WTXC_se), .tx_outp({pad_WTXCpa, pad_WTXCpb}), .tx_outm({pad_WTXCna, pad_WTXCnb}), .iref(ibp_200u_W[1]),
	                          .vtop(), .vbot(), .ctl(), .ctlb());
	
	PDVSS_18_18_NT_DR_H pad026_pin89_vss_W   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_H pad027_pin15_vdd18_W (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVSS_18_18_NT_DR_H pad028_pin89_vss_W   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_H pad029_pin15_vdd18_W (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVSS_18_18_NT_DR_H pad030_pin89_vss_W   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	lvds_tx_top pad031_032_033_034_pin16_17_WTXD (.vddd(vddcore), .vss(vss), .vddio(vddio), .rto(rto), .sns(sns),
	                          .enb(tx_pd_W), .tx_offset_trim(tx_offset_trim_W), .tx_slew(tx_slew_W), 
	                          .tx_in(sig_WTXD_se), .tx_outp({pad_WTXDpa, pad_WTXDpb}), .tx_outm({pad_WTXDna, pad_WTXDnb}), .iref(ibp_200u_W[0]),
	                          .vtop(), .vbot(), .ctl(), .ctlb());
	
	PDVDD_18_18_NT_DR_H pad035_pin18_vdd18_W (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_H  pad036_pin89_vss_W   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVDD_08_08_NT_DR_H  pad037_pin19_vdd08_W (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));

	// GPIO 2 is used for por extern (porx)
	PBIDIR_18_18_NT_DR_H gpio_2_W (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                .PO(), .Y(sig_gpio_2_do), .PAD(pad_gpio_2), .A(sig_gpio_2_di), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                        .IE(tiehi_vddcore), .IS(tielo_vss), .OE(tielo_vss), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVSS_08_08_NT_DR_H  pad039_pin89_vss_W   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVDD_08_08_NT_DR_H  pad040_pin21_vdd08_W (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_H  pad041_pin89_vss_W   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	PBIDIR_18_18_NT_DR_H pad042_pin22_gpio_3 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                          .PO(), .Y(sig_gpio_3_do), .PAD(pad_gpio_3), .A(sig_gpio_3_di), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                                  .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_3_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	///////////////////////////
        ////////// South //////////
	///////////////////////////

	PBIDIR_18_18_NT_DR_V pad043_pin23_gpio_4_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                            .PO(), .Y(sig_gpio_4_do), .PAD(pad_gpio_4), .A(sig_gpio_4_di), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                                    .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_4_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVSS_08_08_NT_DR_V  pad044_pin89_vss_S   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVDD_08_08_NT_DR_V  pad045_pin24_vdd08_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_V  pad046_pin89_vss_S   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	PBIDIR_18_18_NT_DR_V pad047_pin25_gpio_5_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                            .PO(), .Y(sig_gpio_5_do), .PAD(pad_gpio_5), .A(sig_gpio_5_di), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                                    .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_5_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVDD_08_08_NT_DR_V  pad048_pin26_vdd08_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_V  pad049_pin89_vss_S   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_V pad050_pin27_vdd18_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));	
	
	lvds_rx_top pad051_052_pin28_29_SRXD (.vddc(vddcore), .vss(vss), .vddio(vddio), .rto(rto), .sns(sns),
	                          .pd_c(rx_pd_S), .trim_c(tielo_vss), .rx_cdac_p(rx_cdac_p_S), .rx_cdac_m(rx_cdac_m_S),
	                          .rx_bias_trim(rx_bias_trim_S), .rx_rterm_trim(rx_rterm_trim_S), 
	                          .rx_inp(pad_SRXDp), .rx_inm(pad_SRXDn), .rx_out(sig_SRXD_se), .rx_outb(), .ibn_200u(ibn_200u_S[0]), .ibp_50u(ibp_50u_S[0]),
	                          .analog_test({tielo_vss,tielo_vss,Sanalog_test[2:0]}), .testbus_in_1(tb_rx2_1_S), .testbus_in_2(tb_rx2_2_S), .testbus_out_1(tb_rx1_1_S), .testbus_out_2(tb_rx1_2_S),
	                          .testbus_off(tb_off_S), .vref_tg_en(vref_tg_en_S), .vref_in(vref_tb_S),
	                          .ampout_p(), .ampout_m(), .compout_p(), .compout_m());
	
	PDVSS_18_18_NT_DR_V pad053_pin89_vss_S   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_V pad054_pin30_vdd18_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVSS_18_18_NT_DR_V pad055_pin89_vss_S   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_V pad056_pin30_vdd18_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVSS_18_18_NT_DR_V pad057_pin89_vss_S   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	lvds_rx_top pad058_059_pin31_32_SRXC (.vddc(vddcore), .vss(vss), .vddio(vddio), .rto(rto), .sns(sns),
	                          .pd_c(rx_pd_S), .trim_c(tielo_vss), .rx_cdac_p(rx_cdac_p_S), .rx_cdac_m(rx_cdac_m_S),
	                          .rx_bias_trim(rx_bias_trim_S), .rx_rterm_trim(rx_rterm_trim_S), 
	                          .rx_inp(pad_SRXCp), .rx_inm(pad_SRXCn), .rx_out(sig_SRXC_se), .rx_outb(sig_SRXC_seb), .ibn_200u(ibn_200u_S[1]), .ibp_50u(ibp_50u_S[1]),
	                          .analog_test({tielo_vss,Sanalog_test[3],tielo_vss,Sanalog_test[1:0]}), .testbus_in_1(tb_rx1_1_S), .testbus_in_2(tb_rx1_2_S), .testbus_out_1(tb_rx2_1_S), .testbus_out_2(tb_rx2_2_S),
	                          .testbus_off(), .vref_tg_en(), .vref_in(vref_tb_S),
	                          .ampout_p(), .ampout_m(), .compout_p(), .compout_m());
	
	PVSS_08_08_NT_DR_V  pad060_pin89_vss_S   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVDD_08_08_NT_DR_V  pad061_pin33_vdd08_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	ts_pmu  #(.PMU_START_DELAY(PMU_START_DELAY))     pmu_lvds_S (.vddd(vddcore), .vssio(vss), .vddio(vddio),
	                        .pd(pmu_pd_S), .pmu_bias_trim(pmu_bias_trim_S), .vref_tg_en(vref_tg_en_S), .testbus_off(tb_off_S),
	                          .vref(vref_S), .rref(rref_S), .vref_out(vref_tb_S), .ibp_200u(ibp_200u_S), .ibp_50u(ibp_50u_S), .ibn_200u(ibn_200u_S));
	
	ts_por  #(.POR_DELAY(POR_DELAY))    por_S (.vddd(vddcore), .vssio(vss), .vddio(vddio), .por(sig_por));   // MAV (10Mar25) adeded #(.POR_DELAY(POR_DELAY)
	
	PVDD_08_08_NT_DR_V  pad062_pin34_vdd08_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_V  pad063_pin89_vss_S   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	PDVSS_18_18_NT_DR_V dvss10 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_V dvdd10 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	lvds_tx_top pad064_065_066_067_pin35_36_STXC (.vddd(vddcore), .vss(vss), .vddio(vddio), .rto(rto), .sns(sns),
	                          .enb(tx_pd_S), .tx_offset_trim(tx_offset_trim_S), .tx_slew(tx_slew_S), 
	                          .tx_in(sig_STXC_se), .tx_outp({pad_STXCpa, pad_STXCPb}), .tx_outm({pad_STXCna, pad_STXCnb}), .iref(ibp_200u_S[1]),
	                          .vtop(), .vbot(), .ctl(), .ctlb());

	PDVSS_18_18_NT_DR_V pad068_pin89_vss_S   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_V pad069_pin37_vdd18_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVSS_18_18_NT_DR_V pad070_pin89_vss_S   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_V pad071_pin37_vdd18_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_V  pad072_pin89_vss_S   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	lvds_tx_top pad073_074_075_076_pin38_39_STXD (.vddd(vddcore), .vss(vss), .vddio(vddio), .rto(rto), .sns(sns),
	                          .enb(tx_pd_S), .tx_offset_trim(tx_offset_trim_S), .tx_slew(tx_slew_S), 
	                          .tx_in(sig_STXD_se), .tx_outp({pad_STXDpa, pad_STXDpb}), .tx_outm({pad_STXDna, pad_STXDnb}), .iref(ibp_200u_S[0]),
	                          .vtop(), .vbot(), .ctl(), .ctlb());
        
        PVDD_08_08_NT_DR_V  pad077_pin40_vdd18_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_V  pad078_pin89_vss_S   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVDD_08_08_NT_DR_V  pad079_pin41_vdd18_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	PBIDIR_18_18_NT_DR_V pad080_pin42_gpio_6_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                            .PO(), .Y(sig_gpio_6_do), .PAD(pad_gpio_6), .A(sig_gpio_6_di), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                                    .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_6_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVSS_08_08_NT_DR_V  pad081_pin89_vss_S   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVDD_08_08_NT_DR_V  pad082_pin43_vdd08_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_V  pad083_pin89_vss_S   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	PBIDIR_18_18_NT_DR_V pad084_pin44_gpio_7_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                            .PO(), .Y(sig_gpio_7_do), .PAD(pad_gpio_7), .A(sig_gpio_7_di), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                                    .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_7_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	///////////////////////////
        ////////// East ///////////
	///////////////////////////

	PBIDIR_18_18_NT_DR_H pad085_pin45_gpio_8_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                            .PO(), .Y(sig_gpio_8_do), .PAD(pad_gpio_8), .A(sig_gpio_8_di), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                                    .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_8_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVSS_08_08_NT_DR_H  pad086_pin89_vss_E   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVDD_08_08_NT_DR_H  pad087_pin46_vdd08_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_H  pad088_pin89_vss_E   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	PBIDIR_18_18_NT_DR_H pad089_pin47_gpio_9_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                            .PO(), .Y(sig_gpio_9_do), .PAD(pad_gpio_9), .A(sig_gpio_9_di), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                                    .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_9_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVDD_08_08_NT_DR_H  pad090_pin48_vdd08_E  (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_H  pad091_pin89_vss_E    (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_H pad092_pin49_vdd08_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	
	lvds_rx_top pad093_094_pin50_51_ERXD (.vddc(vddcore), .vss(vss), .vddio(vddio), .rto(rto), .sns(sns),
	                          .pd_c(rx_pd_E), .trim_c(tielo_vss), .rx_cdac_p(rx_cdac_p_E), .rx_cdac_m(rx_cdac_m_E),
	                          .rx_bias_trim(rx_bias_trim_E), .rx_rterm_trim(rx_rterm_trim_E), 
	                          .rx_inp(pad_ERXDp), .rx_inm(pad_ERXDn), .rx_out(sig_ERXD_se), .rx_outb(), .ibn_200u(ibn_200u_E[0]), .ibp_50u(ibp_50u_E[0]),
	                          .analog_test({tielo_vss,tielo_vss,Eanalog_test[2:0]}), .testbus_in_1(tb_rx2_1_E), .testbus_in_2(tb_rx2_2_E), .testbus_out_1(tb_rx1_1_E), .testbus_out_2(tb_rx1_2_E),
	                          .testbus_off(tb_off_E), .vref_tg_en(vref_tg_en_E), .vref_in(vref_tb_E),
	                          .ampout_p(), .ampout_m(), .compout_p(), .compout_m());
	
	PDVSS_18_18_NT_DR_H pad095_pin89_vss_E   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_H pad096_pin52_vdd18_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVSS_18_18_NT_DR_H pad097_pin89_vss_E   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_H pad098_pin52_vdd18_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVSS_18_18_NT_DR_H pad099_pin89_vss_E   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	lvds_rx_top pad100_101_pin53_54_ERXC (.vddc(vddcore), .vss(vss), .vddio(vddio), .rto(rto), .sns(sns),
	                          .pd_c(rx_pd_E), .trim_c(tielo_vss), .rx_cdac_p(rx_cdac_p_E), .rx_cdac_m(rx_cdac_m_E),
	                          .rx_bias_trim(rx_bias_trim_E), .rx_rterm_trim(rx_rterm_trim_E), 
	                          .rx_inp(pad_ERXCp), .rx_inm(pad_ERXCn), .rx_out(sig_ERXC_se), .rx_outb(sig_ERXC_seb), .ibn_200u(ibn_200u_E[1]), .ibp_50u(ibp_50u_E[1]),
	                          .analog_test({tielo_vss,Eanalog_test[3],tielo_vss,Eanalog_test[1:0]}), .testbus_in_1(tb_rx1_1_E), .testbus_in_2(tb_rx1_2_E), .testbus_out_1(tb_rx2_1_E), .testbus_out_2(tb_rx2_2_E),
	                          .testbus_off(), .vref_tg_en(), .vref_in(vref_tb_E),
	                          .ampout_p(), .ampout_m(), .compout_p(), .compout_m());
	
	PVSS_08_08_NT_DR_H  pad102_pin89_vss_E   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVDD_08_08_NT_DR_H  pad103_pin55_vdd08_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	
	ts_pmu  #(.PMU_START_DELAY(PMU_START_DELAY))     pmu_lvds_E (.vddd(vddcore), .vssio(vss), .vddio(vddio),
	                          .pd(pmu_pd_E), .pmu_bias_trim(pmu_bias_trim_E), .vref_tg_en(vref_tg_en_E), .testbus_off(tb_off_E),
	                          .vref(vref_E), .rref(rref_E), .vref_out(vref_tb_E), .ibp_200u(ibp_200u_E), .ibp_50u(ibp_50u_E), .ibn_200u(ibn_200u_E));
	
	PVDD_08_08_NT_DR_H  pad104_pin56_vdd08_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_H  pad105_pin89_vss_E   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	
	lvds_tx_top pad106_107_pin57_58_ETXC (.vddd(vddcore), .vss(vss), .vddio(vddio), .rto(rto), .sns(sns),
	                          .enb(tx_pd_E), .tx_offset_trim(tx_offset_trim_E), .tx_slew(tx_slew_E), 
	                          .tx_in(sig_ETXC_se), .tx_outp({pad_ETXCpa, pad_ETXCpb}), .tx_outm({pad_ETXCna, pad_ETXCnb}), .iref(ibp_200u_E[1]),
	                          .vtop(), .vbot(), .ctl(), .ctlb());
        
        PDVSS_18_18_NT_DR_H pad110_pin89_vss_E   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_H pad111_pin59_vdd18_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVSS_18_18_NT_DR_H pad112_pin89_vss_E   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_H pad113_pin59_vdd18_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVSS_18_18_NT_DR_H pad114_pin89_vss_E   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	lvds_tx_top pad115_116_117_118_pin60_61_62_63_ETXD (.vddd(vddcore), .vss(vss), .vddio(vddio), .rto(rto), .sns(sns),
	                        .enb(tx_pd_E), .tx_offset_trim(tx_offset_trim_E), .tx_slew(tx_slew_E), 
	                          .tx_in(sig_ETXD_se), .tx_outp({pad_ETXDpa, pad_ETXDpb}), .tx_outm({pad_ETXDna, pad_ETXDnb}), .iref(ibp_200u_E[0]),
	                          .vtop(), .vbot(), .ctl(), .ctlb());
	
	PDVDD_18_18_NT_DR_H pad119_pin62_vdd18_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_H  pad120_pin89_vss_E   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVDD_08_08_NT_DR_H  pad121_pin63_vdd08_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	// GPIO 10 is used for por bypass (porb)
	PBIDIR_18_18_NT_DR_H pad122_pin64_gpio_10_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                             .PO(), .Y(sig_gpio_10_do), .PAD(pad_gpio_10), .A(sig_gpio_10_di), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                                     .IE(tiehi_vddcore), .IS(tielo_vss), .OE(tielo_vss), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVSS_08_08_NT_DR_H  pad123_pin89_vss_E   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVDD_08_08_NT_DR_H  pad124_pin65_vdd08_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_H  pad125_pin89_vss_E   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	PBIDIR_18_18_NT_DR_H pad126_pin66_gpio_11_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                             .PO(), .Y(sig_gpio_11_do), .PAD(pad_gpio_11), .A(sig_gpio_11_di), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                                     .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_11_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	///////////////////////////
    ////////// North///////////
	///////////////////////////

	PBIDIR_18_18_NT_DR_V pad127_pin67_gpio_12_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                             .PO(), .Y(sig_gpio_12_do), .PAD(pad_gpio_12), .A(sig_gpio_12_di), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                                     .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_12_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVSS_08_08_NT_DR_V  pad128_pin89_vss_N   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVDD_08_08_NT_DR_V  pad129_pin68_vdd08_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_V  pad130_pin89_vss_N   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	PBIDIR_18_18_NT_DR_V pad131_pin69_gpio_13_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                             .PO(), .Y(sig_gpio_13_do), .PAD(pad_gpio_13), .A(sig_gpio_13_di), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                                     .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_13_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVDD_08_08_NT_DR_V  pad132_pin70_vdd08_N  (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_V  pad133_pin89_vss_N    (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_V pad134_pin71_vdd18_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	
	lvds_rx_top pad135_136_pin72_73_NRXD (.vddc(vddcore), .vss(vss), .vddio(vddio), .rto(rto), .sns(sns),
	                        .pd_c(rx_pd_N), .trim_c(tielo_vss), .rx_cdac_p(rx_cdac_p_N), .rx_cdac_m(rx_cdac_m_N),
	                          .rx_bias_trim(rx_bias_trim_N), .rx_rterm_trim(rx_rterm_trim_N), 
	                          .rx_inp(pad_NRXDp), .rx_inm(pad_NRXDn), .rx_out(sig_NRXD_se), .rx_outb(), .ibn_200u(ibn_200u_N[0]), .ibp_50u(ibp_50u_N[0]),
	                          .analog_test({tielo_vss,tielo_vss,Nanalog_test[2:0]}), .testbus_in_1(tb_rx1_1_N), .testbus_in_2(tb_rx1_2_N), .testbus_out_1(tb_rx1_1_N), .testbus_out_2(tb_rx1_2_N),
	                          .testbus_off(tb_off_N), .vref_tg_en(vref_tg_en_N), .vref_in(vref_tb_N),
	                          .ampout_p(), .ampout_m(), .compout_p(), .compout_m());
	
	PDVSS_18_18_NT_DR_V pad137_pin89_vss_N   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_V pad138_pin74_vdd18_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVSS_18_18_NT_DR_V pad139_pin89_vss_N   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_V pad140_pin74_vdd18_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVSS_18_18_NT_DR_V pad141_pin89_vss_N   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	lvds_rx_top pad142_143_pin75_76_NRXC (.vddc(vddcore), .vss(vss), .vddio(vddio), .rto(rto), .sns(sns),
	                          .pd_c(rx_pd_N), .trim_c(tielo_vss), .rx_cdac_p(rx_cdac_p_N), .rx_cdac_m(rx_cdac_m_N),
	                          .rx_bias_trim(rx_bias_trim_N), .rx_rterm_trim(rx_rterm_trim_N), 
	                          .rx_inp(pad_NRXCp), .rx_inm(pad_NRXCn), .rx_out(sig_NRXC_se), .rx_outb(sig_NRXC_seb), .ibn_200u(ibn_200u_N[1]), .ibp_50u(ibp_50u_N[1]),
	                          .analog_test({tielo_vss,Nanalog_test[3],tielo_vss,Nanalog_test[1:0]}), .testbus_in_1(tb_rx1_1_N), .testbus_in_2(tb_rx1_2_N), .testbus_out_1(tb_rx1_1_N), .testbus_out_2(tb_rx1_2_N),
	                          .testbus_off(), .vref_tg_en(), .vref_in(vref_tb_N),
	                          .ampout_p(), .ampout_m(), .compout_p(), .compout_m());
	
	
	PVSS_08_08_NT_DR_V  pad144_pin89_vss_N    (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVDD_08_08_NT_DR_V  pad145_pin77_vdd08_N  (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	ts_pmu  #(.PMU_START_DELAY(PMU_START_DELAY)) pmu_lvds_N (.vddd(vddcore), .vssio(vss), .vddio(vddio),
	                          .pd(pmu_pd_N), .pmu_bias_trim(pmu_bias_trim_N), .vref_tg_en(vref_tg_en_N), .testbus_off(tb_off_N),
	                          .vref(vref_N), .rref(rref_N), .vref_out(vref_tb_N), .ibp_200u(ibp_200u_N), .ibp_50u(ibp_50u_N), .ibn_200u(ibn_200u_N));
	                          
	
	PBIDIR_18_18_NT_DR_V pad146_jtag_tck_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                 .PO(), .Y(sig_jtag_tck), .PAD(pad_jtag_tck), .A(), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                         .IE(tiehi_vddcore), .IS(tielo_vss), .OE(tielo_vss), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PBIDIR_18_18_NT_DR_V pad147_jtag_tdo_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                 .PO(), .Y(), .PAD(pad_jtag_tdo), .A(sig_jtag_tdo), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                         .IE(tiehi_vddcore), .IS(tielo_vss), .OE(tiehi_vddcore), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PBIDIR_18_18_NT_DR_V pad148_jtag_tdi_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                 .PO(), .Y(sig_jtag_tdi), .PAD(pad_jtag_tdi), .A(), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                         .IE(tiehi_vddcore), .IS(tielo_vss), .OE(tielo_vss), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	//RKSW - 6/16/2025 - CHanged TMS pad pull to weak pullup by changing PS and PE to tie hi.
	PBIDIR_18_18_NT_DR_V pad150_jtag_tms_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                 .PO(), .Y(sig_jtag_tms), .PAD(pad_jtag_tms), .A(), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                         .IE(tiehi_vddcore), .IS(tielo_vss), .OE(tielo_vss), .PE(tiehi_vddcore), .PS(tiehi_vddcore), .POE(tielo_vss), .SR(tielo_vss));
	
	PBIDIR_18_18_NT_DR_V pad152_jtag_trst_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                  .PO(), .Y(sig_jtag_trst), .PAD(pad_jtag_trst), .A(), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                          .IE(tiehi_vddcore), .IS(tielo_vss), .OE(tielo_vss), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVDD_08_08_NT_DR_V  pad143_pin78_vdd08_N  (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVSS_18_18_NT_DR_V pad154_pin89_vss_N    (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	lvds_tx_top pad155_156_157_158_pin79_80_NTXC (.vddd(vddcore), .vss(vss), .vddio(vddio), .rto(rto), .sns(sns),
	                          .enb(tx_pd_N), .tx_offset_trim(tx_offset_trim_N), .tx_slew(tx_slew_N), 
	                          .tx_in(sig_NTXC_se), .tx_outp({pad_NTXCpa, pad_NTXCpb}), .tx_outm({pad_NTXCna, pad_EXTCnb}), .iref(ibp_200u_N[1]),
	                          .vtop(), .vbot(), .ctl(), .ctlb());

	PDVSS_18_18_NT_DR_V pad159_pin89_vss_N   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_V pad160_pin81_vdd18_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVSS_18_18_NT_DR_V pad161_pin89_vss_N   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVDD_18_18_NT_DR_V pad162_pin81_vdd18_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PDVSS_18_18_NT_DR_V pad163_pin89_vss_N   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	lvds_tx_top pad164_165_166_167_pin82_83_NTXD (.vddd(vddcore), .vss(vss), .vddio(vddio), .rto(rto), .sns(sns),
	                          .enb(tx_pd_N), .tx_offset_trim(tx_offset_trim_N), .tx_slew(tx_slew_N), 
	                          .tx_in(sig_NTXD_se), .tx_outp({pad_NTXDpa, pad_NTXDpb}), .tx_outm({pad_NTXDna, pad_NTXDnb}), .iref(ibp_200u_N[0]),
	                          .vtop(), .vbot(), .ctl(), .ctlb());
	
	PDVDD_18_18_NT_DR_V pad168_pin84_vdd18_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_V  pad169_pin89_vss_N   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVDD_08_08_NT_DR_V  pad170_pin85_vdd08_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	PBIDIR_18_18_NT_DR_V pad171_pin86_gpio_14_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                             .PO(), .Y(sig_gpio_14_do), .PAD(pad_gpio_14), .A(sig_gpio_14_di), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                                     .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_14_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVSS_08_08_NT_DR_V  pad172_pin89_vss_N   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVDD_08_08_NT_DR_V  pad173_pin87_vdd08_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	PVSS_08_08_NT_DR_V  pad174_pin89_vss_N   (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns));
	
	PBIDIR_18_18_NT_DR_V pad175_pin88_gpio_15_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(rto), .SNS(sns),
	                                .PO(), .Y(sig_gpio_15_do), .PAD(pad_gpio_15), .A(sig_gpio_15_di), .DS0(tiehi_vddcore), .DS1(tielo_vss), 
                                        .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_15_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));


   // MAV (12Mar25) moved to here & added "assign" plus changed "," tp ";"
   // Differential Outputs
   /* C0mmenetd out because this functionality is in the AFE_models. RK Mar 13, 2025
   assign pad_WTXCpa, pad_a  =  sig_WTXD_se;     assign pad_WTXCna, pad_a = ~sig_WTXD_se;
   assign pad_WTXCpa, pad_b  =  sig_WTXD_se;     assign pad_WTXCna, pad_b = ~sig_WTXD_se;
   assign pad_STXCpa, pad_a  =  sig_STXD_se;     assign pad_STXCna, pad_a = ~sig_STXD_se;
   assign pad_STXCpa, pad_b  =  sig_STXD_se;     assign pad_STXCna, pad_b = ~sig_STXD_se;
   assign pad_ETXCpa, pad_a  =  sig_ETXD_se;     assign pad_ETXCna, pad_a = ~sig_ETXD_se;
   assign pad_ETXCpa, pad_b  =  sig_ETXD_se;     assign pad_ETXCna, pad_b = ~sig_ETXD_se;
   assign pad_NTXCpa, pad_a  =  sig_NTXD_se;     assign pad_NTXCna, pad_a = ~sig_NTXD_se;
   assign pad_NTXCpa, pad_b  =  sig_NTXD_se;     assign pad_NTXCna, pad_b = ~sig_NTXD_se;

   assign pad_WTXDpa  =  sig_WTXD_se;     assign pad_WTXDna = ~sig_WTXD_se;
   assign pad_WTXDpb  =  sig_WTXD_se;     assign pad_WTXDnb = ~sig_WTXD_se;
   assign pad_STXDpa  =  sig_STXD_se;     assign pad_STXDna = ~sig_STXD_se;
   assign pad_STXDpb  =  sig_STXD_se;     assign pad_STXDnb = ~sig_STXD_se;
   assign pad_ETXDpa  =  sig_ETXD_se;     assign pad_ETXDna = ~sig_ETXD_se;
   assign pad_ETXDpb  =  sig_ETXD_se;     assign pad_ETXDnb = ~sig_ETXD_se;
   assign pad_NTXDpa  =  sig_NTXD_se;     assign pad_NTXDna = ~sig_NTXD_se;
   assign pad_NTXDpb  =  sig_NTXD_se;     assign pad_NTXDnb = ~sig_NTXD_se;

   // Differential Inputs
   assign sig_WRXC_se =  pad_WRXCp  &&  !pad_WRXCn  ?  1'b1  :  !pad_WRXCp  &&  pad_WRXCn  ?  1'b0 : 1'bx;
   assign sig_WRXD_se =  pad_WRXDp  &&  !pad_WRXDn  ?  1'b1  :  !pad_WRXDp  &&  pad_WRXDn  ?  1'b0 : 1'bx;
   assign sig_SRXC_se =  pad_SRXCp  &&  !pad_SRXCn  ?  1'b1  :  !pad_SRXCp  &&  pad_SRXCn  ?  1'b0 : 1'bx;
   assign sig_SRXD_se =  pad_SRXDp  &&  !pad_SRXDn  ?  1'b1  :  !pad_SRXDp  &&  pad_SRXDn  ?  1'b0 : 1'bx;
   assign sig_ERXC_se =  pad_ERXCp  &&  !pad_ERXCn  ?  1'b1  :  !pad_ERXCp  &&  pad_ERXCn  ?  1'b0 : 1'bx;
   assign sig_ERXD_se =  pad_ERXDp  &&  !pad_ERXDn  ?  1'b1  :  !pad_ERXDp  &&  pad_ERXDn  ?  1'b0 : 1'bx;
   assign sig_NRXC_se =  pad_NRXCp  &&  !pad_NRXCn  ?  1'b1  :  !pad_NRXCp  &&  pad_NRXCn  ?  1'b0 : 1'bx;
   assign sig_NRXD_se =  pad_NRXDp  &&  !pad_NRXDn  ?  1'b1  :  !pad_NRXDp  &&  pad_NRXDn  ?  1'b0 : 1'bx;
   */

endmodule

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
