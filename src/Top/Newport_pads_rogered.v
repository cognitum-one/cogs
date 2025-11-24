//===============================================================================================================================
//
//    Copyright © 2021.2025 Advanced Architectures
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
//    All VSS pads are downbonded to the heatsink slug.   The slug may therefore be considered as pin 89
//
//===============================================================================================================================
`timescale 1ps/1fs

module Cognitum_pads (
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
   input  wire  pad jtag_tdi,
   input  wire  pad_jtag_tms,
   input  wire  pad_jtag_trst,

   // Signals to & from the core
   input  wire  sig_gpio_0_di,   input  wire sig_gpio_4_di,    input  wire sig_gpio_8_di,    input  wire sig_gpio_12_di,
   output wire  sig_gpio_0_do,   output wire sig_gpio_4_do,    output wire sig_gpio_8_do,    output wire sig_gpio_12_do,
   output wire  sig_gpio_0_oe,   output wire sig_gpio_4_oe,    output wire sig_gpio_8_oe,    output wire sig_gpio_12_oe,
   input  wire  sig_gpio_1_di,   input  wire sig_gpio_5_di,    input  wire sig_gpio_9_di,    input  wire sig_gpio_13_di,
   output wire  sig_gpio_1_do,   output wire sig_gpio_5_do,    output wire sig_gpio_9_do,    output wire sig_gpio_13_do,
   output wire  sig_gpio_1_oe,   output wire sig_gpio_5_oe,    output wire sig_gpio_9_oe,    output wire sig_gpio_13_oe,
   output wire  sig_WRXD_se,     output wire sig_SRXD_se,      output wire sig_ERXD_se,      output wire sig_NRXD_se,
   output wire  sig_WRXC_se,     output wire sig_SRXC_se,      output wire sig_ERXC_se,      output wire sig_NRXC_se,
   input  wire  sig_WTXC_se,     input  wire sig_STXC_se,      input  wire sig_ETXC_se,      input  wire sig_NTXC_se,
   input  wire  sig_WTXD_se,     input  wire sig_STXD_se,      input  wire sig_ETXD_se,      input  wire sig_NTXD_se,
   input  wire  sig_gpio_2_di,   input  wire sig_gpio_6_di,    input  wire sig_gpio_10_di,   input  wire sig_gpio_14_di,
   output wire  sig_gpio_2_do,   output wire sig_gpio_6_do,    output wire sig_gpio_10_do,   output wire sig_gpio_14_do,
   output wire  sig_gpio_2_oe,   output wire sig_gpio_6_oe,    output wire sig_gpio_10_oe,   output wire sig_gpio_14_oe,
   input  wire  sig_gpio_3_di,   input  wire sig_gpio_7_di,    input  wire sig_gpio_11_di,   input  wire sig_gpio_15_di,
   output wire  sig_gpio_3_do,   output wire sig_gpio_7_do,    output wire sig_gpio_11_do,   output wire sig_gpio_15_do,
   output wire  sig_gpio_3_oe,   output wire sig_gpio_7_oe,    output wire sig_gpio_11_oe,   output wire sig_gpio_15_oe,
   output wire  sig_jtag_tck,
   input  wire  sig_jtag_tdo,
   output wire  sig jtag_tdi,
   output wire  sig_jtag_tms,
   output wire  sig_jtag_trst,
   // AFE Configuration
   output wire [5:0] Wrx_rterm_trim,         Srx_rterm_trim,               Erx_rterm_trim,               Nrx_rterm_trim,
   output wire [3:0] Wrx_offset_trim,        Srx_offset_trim,              Erx_offset_trim,              Nrx_offset_trim,
   output wire [4:0] Wrx_cdac_p,             Srx_cdac_p,                   Erx_cdac_p,                   Nrx_cdac_p,
   output wire [4:0] Wrx_cdac_m,             Srx_cdac_m,                   Erx_cdac_m,                   Nrx_cdac_m,
   output wire [3:0] Wrx_bias_trim,          Srx_bias_trim,                Erx_bias_trim,                Nrx_bias_trim,
   output wire [3:0] Wrx_gain_trim,          Srx_gain_trim,                Erx_gain_trim,                Nrx_gain_trim,
   output wire [3:0] Wtx_bias_trim,          Stx_bias_trim,                Etx_bias_trim,                Ntx_bias_trim,
   output wire [3:0] Wtx_offset_trim,        Stx_offset_trim,              Etx_offset_trim,              Ntx_offset_trim,
   output wire [5:0] Wpmu_bias_trim,         Spmu_bias_trim,               Epmu_bias_trim,               Npmu_bias_trim,
   output wire [5:0] Wlvds_spare_in,         Slvds_spare_in,               Elvds_spare_in,               Nlvds_spare_in,
   input wire  [7:0] Wlvds_spare_out,        Slvds_spare_out,              Elvds_spare_out,              Nlvds_spare_out,
   output wire [4:0] Wanalog_test,           Sanalog_test,                 Eanalog_test,                 Nanalog_test,
   output wire       Wanalog_reserved,       Sanalog_reserved,             Eanalog_reserved,             Nanalog_reserved,
   output wire       Wpreserve_spare_net,    Spreserve_spare_net,          Epreserve_spare_net,          Npreserve_spare_net
   output wire       Wpower_down,            Spower_down,                  Epower_down,                  Npower_down
);
module Cognitum_pads
  #(parameter PMU_START_DELAY = 100000, 
    parameter POR_DELAY = 1000000)
  (
   // PADS
   inout  wire  pad_gpio_0,      inout  wire  pad_gpio_4,      inout  wire  pad_gpio_8,       inout  wire  pad_gpio_12,
   inout  wire  pad_gpio_1,      inout  wire  pad_gpio_5,      inout  wire  pad_gpio_9,       inout  wire  pad_gpio_13,
   input  wire  pad_WRXDp,       input  wire  pad_SRXDp,       input  wire  pad_ERXDp,        input  wire  pad_NRXDp,
   input  wire  pad_WRXDn,       input  wire  pad_SRXDn,       input  wire  pad_ERXDn,        input  wire  pad_NRXDn,
   input  wire  pad_WRXCp,       input  wire  pad_SRXCp,       input  wire  pad_ERXCp,        input  wire  pad_NRXCp,
   input  wire  pad_WRXCn,       input  wire  pad_SRXCn,       input  wire  pad_ERXCn,        input  wire  pad_NRXCn,
   output wire  pad_WTXCn,       output wire  pad_STXCn,       output wire  pad_ETXCn,        input  wire  pad_jtag_tck,
   output wire  pad_WTXCp,       output wire  pad_STXCp,       output wire  pad_ETXCp,        output wire  pad_jtag_tdo,
   output wire  pad_WTXDn,       output wire  pad_STXDn,       output wire  pad_ETXDn,        input  wire  pad_jtag_tdi,
   output wire  pad_WTXDp,       output wire  pad_STXDp,       output wire  pad_ETXDp,        input  wire  pad_jtag_tms,
   inout  wire  pad_gpio_2,      inout  wire  pad_gpio_6,      inout  wire  pad_gpio_10,      input  wire  pad_jtag_trst,
   inout  wire  pad_gpio_3,      inout  wire  pad_gpio_7,      inout  wire  pad_gpio_11,      output wire  pad_NTXCn,
                                                                                              output wire  pad_NTXCp,
                                                                                              output wire  pad_NTXDn,
                                                                                              output wire  pad_NTXDp,
                                                                                              inout  wire  pad_gpio_14,
                                                                                              inout  wire  pad_gpio_15,
   // Signals to & from the core
   input  wire  sig_gpio_0_di,   input  wire  sig_gpio_4_di,   input  wire  sig_gpio_8_di,    input  wire  sig_gpio_12_di,
   output wire  sig_gpio_0_do,   output wire  sig_gpio_4_do,   output wire  sig_gpio_8_do,    output wire  sig_gpio_12_do,
   input  wire  sig_gpio_0_oe,   input  wire  sig_gpio_4_oe,   input  wire  sig_gpio_8_oe,    input  wire  sig_gpio_12_oe,
   input  wire  sig_gpio_1_di,   input  wire  sig_gpio_5_di,   input  wire  sig_gpio_9_di,    input  wire  sig_gpio_13_di,
   output wire  sig_gpio_1_do,   output wire  sig_gpio_5_do,   output wire  sig_gpio_9_do,    output wire  sig_gpio_13_do,
   input  wire  sig_gpio_1_oe,   input  wire  sig_gpio_5_oe,   input  wire  sig_gpio_9_oe,    input  wire  sig_gpio_13_oe,
   output wire  sig_WRXD_se,     output wire  sig_SRXD_se,     output wire  sig_ERXD_se,      output wire  sig_NRXD_se,
   output wire  sig_WRXC_se,     output wire  sig_SRXC_se,     output wire  sig_ERXC_se,      output wire  sig_NRXC_se,
   output wire  sig_WRXC_seb,    output wire  sig_SRXC_seb,    output wire  sig_ERXC_seb,     output wire  sig_NRXC_seb,
   input  wire  sig_WTXC_se,     input  wire  sig_STXC_se,     input  wire  sig_ETXC_se,      output wire  sig_jtag_tck,
   input  wire  sig_WTXD_se,     input  wire  sig_STXD_se,     input  wire  sig_ETXD_se,      input  wire  sig_jtag_tdo,
   input  wire  sig_gpio_2_di,   input  wire  sig_gpio_6_di,   input  wire  sig_gpio_10_di,   output wire  sig_jtag_tdi,
   output wire  sig_gpio_2_do,   output wire  sig_gpio_6_do,   output wire  sig_gpio_10_do,   output wire  sig_jtag_tms,
   input  wire  sig_gpio_2_oe,   input  wire  sig_gpio_6_oe,   input  wire  sig_gpio_10_oe,   output wire  sig_jtag_trst,
   input  wire  sig_gpio_3_di,   input  wire  sig_gpio_7_di,   input  wire  sig_gpio_11_di,   input  wire  sig_NTXC_se,
   output wire  sig_gpio_3_do,   output wire  sig_gpio_7_do,   output wire  sig_gpio_11_do,   input  wire  sig_NTXD_se,
   input  wire  sig_gpio_3_oe,   input  wire  sig_gpio_7_oe,   input  wire  sig_gpio_11_oe,   input  wire  sig_gpio_14_di,
                                 output wire  sig_por,                                        output wire  sig_gpio_14_do,
                                                                                              input  wire  sig_gpio_14_oe,
                                                                                              input  wire  sig_gpio_15_di,
                                                                                              output wire  sig_gpio_15_do,
                                                                                              input  wire  sig_gpio_15_oe,
																							  																							  
   input wire [4:0] rx_cdac_p_W, rx_cdac_p_S, rx_cdac_p_E, rx_cdac_p_N,
   input wire [4:0] rx_cdac_m_W, rx_cdac_m_S, rx_cdac_m_E, rx_cdac_m_N,
   input wire [3:0] rx_bias_trim_W, rx_bias_trim_S, rx_bias_trim_E, rx_bias_trim_N,
   input wire [3:0] rx_offset_trim_W, rx_offset_trim_S, rx_offset_trim_E, rx_offset_trim_N,
   input wire [3:0] rx_gain_trim_W, rx_gain_trim_S, rx_gain_trim_E, rx_gain_trim_N,
   input wire [5:0] rx_rterm_trim_W, rx_rterm_trim_S, rx_rterm_trim_E, rx_rterm_trim_N,
   input wire [1:0] tx_slew_W, tx_slew_S, tx_slew_E, tx_slew_N,
   input wire [3:0] tx_bias_trim_W, tx_bias_trim_S, tx_bias_trim_E, tx_bias_trim_N,
   input wire [3:0] tx_offset_trim_W, tx_offset_trim_S, tx_offset_trim_E, tx_offset_trim_N,
   input wire [5:0] pmu_bias_trim_W, pmu_bias_trim_S, pmu_bias_trim_E, pmu_bias_trim_N,
   input wire pmu_pd_W, pmu_pd_S, pmu_pd_E, pmu_pd_N,
   input wire rx_pd_W, rx_pd_S, rx_pd_E, rx_pd_N,
   input wire tx_pd_W, tx_pd_S, tx_pd_E, tx_pd_N,

   inout wire vddcore, vddio, vss, rto, sns

   );

   
	reg tiehi;
	wire tiehi_vddio;
	assign tiehi_vddio = tiehi;
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
   
	wire [1:0] ibp_50u_W, ibp_50u_S, ibp_50u_E, ibp_50u_N;
	wire [1:0] ibn_200u_W, ibn_200u_S, ibn_200u_E, ibn_200u_N;
	wire [1:0] ibp_200u_W, ibp_200u_S, ibp_200u_E, ibp_200u_N;

	///////////////////////////
    ////////// West //////////
	///////////////////////////

	PBIDIR_18_18_NT_DR_H gpio_0_W (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_gpio_0_do), .PAD(pad_gpio_0), .A(sig_gpio_0_di), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_0_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVSS_08_08_NT_DR_H  vss1 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVDD_08_08_NT_DR_H  vdd1 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_H  vss2 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	PBIDIR_18_18_NT_DR_H gpio_1_W (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_gpio_1_do), .PAD(pad_gpio_1), .A(sig_gpio_1_di), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_1_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVDD_08_08_NT_DR_H  vdd2 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_H  vss3 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVDD_18_18_NT_DR_H dvdd1 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVSS_18_18_NT_DR_H dvss1 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	lvds_rx_top lvds_rx_WRXD (.vddc(vddcore), .vss(vss), .vddio(vddio), .rto(tielo_vss), .sns(tiehi_vddio),
	                        .pd_c(rx_pd_W), .trim_c(tielo_vss), .rx_cdac_p(rx_cdac_p_W), .rx_cdac_m(rx_cdac_m_W),
							.rx_bias_trim(rx_bias_trim_W), .rx_offset_trim(rx_offset_trim_W), .rx_gain_trim(), .rx_rterm_trim(rx_rterm_trim_W), 
							.rx_inp(pad_WRXDp), .rx_inm(pad_WRXDn), .rx_out(sig_WRXD_se), .rx_outb(), .ibn_200u(ibn_200u_W[0]), .ibp_50u(ibp_50u_W[0]),
							.analog_test(), .testbus_in_1(tb_rx2_1_W), .testbus_in_2(tb_rx2_2_W), .testbus_out_1(tb_rx1_1_W), .testbus_out_2(tb_rx1_2_W),
							.testbus_off(tb_off_W), .vref_tg_en(vref_tg_en_W), .vref_in(vref_tb_W),
							.ampout_p(), .ampout_m(), .compout_p(), .compout_m());
	
	PDVDD_18_18_NT_DR_H dvdd2 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVSS_18_18_NT_DR_H dvss2 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	lvds_rx_top lvds_rx_WRXC (.vddc(vddcore), .vss(vss), .vddio(vddio), .rto(tielo_vss), .sns(tiehi_vddio),
	                        .pd_c(rx_pd_W), .trim_c(tielo_vss), .rx_cdac_p(rx_cdac_p_W), .rx_cdac_m(rx_cdac_m_W),
							.rx_bias_trim(rx_bias_trim_W), .rx_offset_trim(rx_offset_trim_W), .rx_gain_trim(), .rx_rterm_trim(rx_rterm_trim_W), 
							.rx_inp(pad_WRXCp), .rx_inm(pad_WRXCn), .rx_out(sig_WRXC_se), .rx_outb(sig_WRXC_seb), .ibn_200u(ibn_200u_W[1]), .ibp_50u(ibp_50u_W[1]),
							.analog_test(), .testbus_in_1(tb_rx1_1_W), .testbus_in_2(tb_rx1_2_W), .testbus_out_1(tb_rx2_1_W), .testbus_out_2(tb_rx2_2_W),
							.testbus_off(tb_off_W), .vref_tg_en(vref_tg_en_W), .vref_in(vref_tb_W),
							.ampout_p(), .ampout_m(), .compout_p(), .compout_m());
	
	PDVDD_18_18_NT_DR_H dvdd3 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVSS_18_18_NT_DR_H dvss3 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVDD_08_08_NT_DR_H  vdd3 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVDD_08_08_NT_DR_H  vdd4 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_H  vss4 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	ts_pmu  #(.PMU_START_DELAY(PMU_START_DELAY)) pmu_lvds_W (.vddd(vddcore), .vssio(vss), .vddio(vddio),
	                        .pd(pmu_pd_W), .pmu_bias_trim(pmu_bias_trim_W), .vref_tg_en(vref_tg_en_W), .testbus_off(tb_off_W),
				.vref(vref_W), .rref(rref_W), .vref_out(vref_tb_W), .ibp_200u(ibp_200u_W), .ibp_50u(ibp_50u_W), .ibn_200u(ibn_200u_W));
	
	PDVSS_18_18_NT_DR_H dvss4 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVDD_18_18_NT_DR_H dvdd4 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	lvds_tx_top lvds_tx_WTXC (.vddd(vddcore), .vss(vss), .vddio(vddio), .rto(tielo_vss), .sns(tiehi_vddio),
	                        .enb(tx_pd_W), .tx_offset_trim(tx_offset_trim_W), .tx_bias_trim(tx_bias_trim_W), .tx_slew(tx_slew_W), 
							.tx_in(sig_WTXC_se), .tx_clk_outp(pad_WTXCp), .tx_clk_outm(pad_WTXCn), .iref(ibp_200u_W[1]),
							.vtop(vtop_WC), .vbot(vbot_WC), .ctl(ctl_WC), .ctlb(ctlb_WC));

	PDVSS_18_18_NT_DR_H dvss5 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVDD_18_18_NT_DR_H dvdd5 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	lvds_tx_top lvds_tx_WTXD (.vddd(vddcore), .vss(vss), .vddio(vddio), .rto(tielo_vss), .sns(tiehi_vddio),
	                        .enb(tx_pd_W), .tx_offset_trim(tx_offset_trim_W), .tx_bias_trim(tx_bias_trim_W), .tx_slew(tx_slew_W), 
				.tx_in(sig_WTXD_se), .tx_clk_outp(pad_WTXDp), .tx_clk_outm(pad_WTXDn), .iref(ibp_200u_W[0]),
				.vtop(vtop_WD), .vbot(vbot_WD), .ctl(ctl_WD), .ctlb(ctlb_WD));
	
	PDVSS_18_18_NT_DR_H dvss6 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVDD_18_18_NT_DR_H dvdd6 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_H  vss5 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	PBIDIR_18_18_NT_DR_H gpio_2_W (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_gpio_2_do), .PAD(pad_gpio_2), .A(sig_gpio_2_di), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_2_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVSS_08_08_NT_DR_H  vss6 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVDD_08_08_NT_DR_H  vdd5 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_H  vss7 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	PBIDIR_18_18_NT_DR_H gpio_3_W (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_gpio_3_do), .PAD(pad_gpio_3), .A(sig_gpio_3_di), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_3_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	///////////////////////////
    ////////// South //////////
	///////////////////////////

	PBIDIR_18_18_NT_DR_V gpio_4_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_gpio_4_do), .PAD(pad_gpio_4), .A(sig_gpio_4_di), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_4_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVSS_08_08_NT_DR_V  vss8 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVDD_08_08_NT_DR_V  vdd6 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_V  vss9 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	PBIDIR_18_18_NT_DR_V gpio_5_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_gpio_5_do), .PAD(pad_gpio_5), .A(sig_gpio_5_di), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_5_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVDD_08_08_NT_DR_V  vdd7 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_V  vss10 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVDD_18_18_NT_DR_V dvdd7 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVSS_18_18_NT_DR_V dvss7 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	lvds_rx_top lvds_rx_SRXD (.vddc(vddcore), .vss(vss), .vddio(vddio), .rto(tielo_vss), .sns(tiehi_vddio),
	                        .pd_c(rx_pd_S), .trim_c(tielo_vss), .rx_cdac_p(rx_cdac_p_S), .rx_cdac_m(rx_cdac_m_S),
							.rx_bias_trim(rx_bias_trim_S), .rx_offset_trim(rx_offset_trim_S), .rx_gain_trim(), .rx_rterm_trim(rx_rterm_trim_S), 
							.rx_inp(pad_SRXDp), .rx_inm(pad_SRXDn), .rx_out(sig_SRXD_se), .rx_outb(), .ibn_200u(ibn_200u_S[0]), .ibp_50u(ibp_50u_S[0]),
							.analog_test(), .testbus_in_1(tb_rx2_1_S), .testbus_in_2(tb_rx2_2_S), .testbus_out_1(tb_rx1_1_S), .testbus_out_2(tb_rx1_2_S),
							.testbus_off(tb_off_S), .vref_tg_en(vref_tg_en_S), .vref_in(vref_tb_S),
							.ampout_p(), .ampout_m(), .compout_p(), .compout_m());
	
	PDVDD_18_18_NT_DR_V dvdd8 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVSS_18_18_NT_DR_V dvss8 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	lvds_rx_top lvds_rx_SRXC (.vddc(vddcore), .vss(vss), .vddio(vddio), .rto(tielo_vss), .sns(tiehi_vddio),
	                        .pd_c(rx_pd_S), .trim_c(tielo_vss), .rx_cdac_p(rx_cdac_p_S), .rx_cdac_m(rx_cdac_m_S),
							.rx_bias_trim(rx_bias_trim_S), .rx_offset_trim(rx_offset_trim_S), .rx_gain_trim(), .rx_rterm_trim(rx_rterm_trim_S), 
							.rx_inp(pad_SRXCp), .rx_inm(pad_SRXCn), .rx_out(sig_SRXC_se), .rx_outb(sig_SRXC_seb), .ibn_200u(ibn_200u_S[1]), .ibp_50u(ibp_50u_S[1]),
							.analog_test(), .testbus_in_1(tb_rx1_1_S), .testbus_in_2(tb_rx1_2_S), .testbus_out_1(tb_rx2_1_S), .testbus_out_2(tb_rx2_2_S),
							.testbus_off(tb_off_S), .vref_tg_en(vref_tg_en_S), .vref_in(vref_tb_S),
							.ampout_p(), .ampout_m(), .compout_p(), .compout_m());
	
	PDVDD_18_18_NT_DR_V dvdd9 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVSS_18_18_NT_DR_V dvss9 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVDD_08_08_NT_DR_V  vdd8  (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVDD_08_08_NT_DR_V  vdd9  (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_V  vss11 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	ts_pmu  #(.PMU_START_DELAY(PMU_START_DELAY))     pmu_lvds_S (.vddd(vddcore), .vssio(vss), .vddio(vddio),
	                        .pd(pmu_pd_S), .pmu_bias_trim(pmu_bias_trim_S), .vref_tg_en(vref_tg_en_S), .testbus_off(tb_off_S),
							.vref(vref_S), .rref(rref_S), .vref_out(vref_tb_S), .ibp_200u(ibp_200u_S), .ibp_50u(ibp_50u_S), .ibn_200u(ibn_200u_S));
	
	ts_por  #(.POR_DELAY(POR_DELAY))    por_S (.vddd(vddcore), .vssio(vss), .vddio(vddio), .por(sig_por));   // MAV (10Mar25) adeded #(.POR_DELAY(POR_DELAY)
	
	PDVSS_18_18_NT_DR_V dvss10 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVDD_18_18_NT_DR_V dvdd10 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	lvds_tx_top lvds_tx_STXC (.vddd(vddcore), .vss(vss), .vddio(vddio), .rto(tielo_vss), .sns(tiehi_vddio),
	                        .enb(tx_pd_S), .tx_offset_trim(tx_offset_trim_S), .tx_bias_trim(tx_bias_trim_S), .tx_slew(tx_slew_S), 
							.tx_in(sig_STXC_se), .tx_clk_outp(pad_STXCp), .tx_clk_outm(pad_STXCn), .iref(ibp_200u_S[1]),
							.vtop(vtop_SC), .vbot(vbot_SC), .ctl(ctl_SC), .ctlb(ctlb_SC));

	PDVSS_18_18_NT_DR_V dvss11 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVDD_18_18_NT_DR_V dvdd11 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	lvds_tx_top lvds_tx_STXD (.vddd(vddcore), .vss(vss), .vddio(vddio), .rto(tielo_vss), .sns(tiehi_vddio),
	                        .enb(tx_pd_S), .tx_offset_trim(tx_offset_trim_S), .tx_bias_trim(tx_bias_trim_S), .tx_slew(tx_slew_S), 
							.tx_in(sig_STXD_se), .tx_clk_outp(pad_STXDp), .tx_clk_outm(pad_STXDn), .iref(ibp_200u_S[0]),
							.vtop(vtop_SD), .vbot(vbot_SD), .ctl(ctl_SD), .ctlb(ctlb_SD));
	
	PDVSS_18_18_NT_DR_V dvss12 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVDD_18_18_NT_DR_V dvdd12 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_V  vss12  (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	PBIDIR_18_18_NT_DR_V gpio_6_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_gpio_6_do), .PAD(pad_gpio_6), .A(sig_gpio_6_di), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_6_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVSS_08_08_NT_DR_V  vss13 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVDD_08_08_NT_DR_V  vdd10 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_V  vss14 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	PBIDIR_18_18_NT_DR_V gpio_7_S (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_gpio_7_do), .PAD(pad_gpio_7), .A(sig_gpio_7_di), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_7_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	///////////////////////////
    ////////// East ///////////
	///////////////////////////

	PBIDIR_18_18_NT_DR_H gpio_8_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_gpio_8_do), .PAD(pad_gpio_8), .A(sig_gpio_8_di), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_8_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVSS_08_08_NT_DR_H  vss15 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVDD_08_08_NT_DR_H  vdd11 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_H  vss16 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	PBIDIR_18_18_NT_DR_H gpio_9_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_gpio_9_do), .PAD(pad_gpio_9), .A(sig_gpio_9_di), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_9_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVDD_08_08_NT_DR_H  vdd12  (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_H  vss17  (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVDD_18_18_NT_DR_H dvdd13 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVSS_18_18_NT_DR_H dvss13 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	lvds_rx_top lvds_rx_ERXD (.vddc(vddcore), .vss(vss), .vddio(vddio), .rto(tielo_vss), .sns(tiehi_vddio),
	                        .pd_c(rx_pd_E), .trim_c(tielo_vss), .rx_cdac_p(rx_cdac_p_E), .rx_cdac_m(rx_cdac_m_E),
							.rx_bias_trim(rx_bias_trim_E), .rx_offset_trim(rx_offset_trim_E), .rx_gain_trim(), .rx_rterm_trim(rx_rterm_trim_E), 
							.rx_inp(pad_ERXDp), .rx_inm(pad_ERXDn), .rx_out(sig_ERXD_se), .rx_outb(), .ibn_200u(ibn_200u_E[0]), .ibp_50u(ibp_50u_E[0]),
							.analog_test(), .testbus_in_1(tb_rx2_1_E), .testbus_in_2(tb_rx2_2_E), .testbus_out_1(tb_rx1_1_E), .testbus_out_2(tb_rx1_2_E),
							.testbus_off(tb_off_E), .vref_tg_en(vref_tg_en_E), .vref_in(vref_tb_E),
							.ampout_p(), .ampout_m(), .compout_p(), .compout_m());
	
	PDVDD_18_18_NT_DR_H dvdd14 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVSS_18_18_NT_DR_H dvss14 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	lvds_rx_top lvds_rx_ERXC (.vddc(vddcore), .vss(vss), .vddio(vddio), .rto(tielo_vss), .sns(tiehi_vddio),
	                        .pd_c(rx_pd_E), .trim_c(tielo_vss), .rx_cdac_p(rx_cdac_p_E), .rx_cdac_m(rx_cdac_m_E),
							.rx_bias_trim(rx_bias_trim_E), .rx_offset_trim(rx_offset_trim_E), .rx_gain_trim(), .rx_rterm_trim(rx_rterm_trim_E), 
							.rx_inp(pad_ERXCp), .rx_inm(pad_ERXCn), .rx_out(sig_ERXC_se), .rx_outb(sig_ERXC_seb), .ibn_200u(ibn_200u_E[1]), .ibp_50u(ibp_50u_E[1]),
							.analog_test(), .testbus_in_1(tb_rx1_1_E), .testbus_in_2(tb_rx1_2_E), .testbus_out_1(tb_rx2_1_E), .testbus_out_2(tb_rx2_2_E),
							.testbus_off(tb_off_E), .vref_tg_en(vref_tg_en_E), .vref_in(vref_tb_E),
							.ampout_p(), .ampout_m(), .compout_p(), .compout_m());
	
	PDVDD_18_18_NT_DR_H dvdd15 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVSS_18_18_NT_DR_H dvss15 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVDD_08_08_NT_DR_H  vdd13 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVDD_08_08_NT_DR_H  vdd14 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_H  vss18 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	ts_pmu  #(.PMU_START_DELAY(PMU_START_DELAY))     pmu_lvds_E (.vddd(vddcore), .vssio(vss), .vddio(vddio),
	                        .pd(pmu_pd_E), .pmu_bias_trim(pmu_bias_trim_E), .vref_tg_en(vref_tg_en_E), .testbus_off(tb_off_E),
							.vref(vref_E), .rref(rref_E), .vref_out(vref_tb_E), .ibp_200u(ibp_200u_E), .ibp_50u(ibp_50u_E), .ibn_200u(ibn_200u_E));
	
	PDVSS_18_18_NT_DR_H dvss16 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVDD_18_18_NT_DR_H dvdd16 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	lvds_tx_top lvds_tx_ETXC (.vddd(vddcore), .vss(vss), .vddio(vddio), .rto(tielo_vss), .sns(tiehi_vddio),
	                        .enb(tx_pd_E), .tx_offset_trim(tx_offset_trim_E), .tx_bias_trim(tx_bias_trim_E), .tx_slew(tx_slew_E), 
							.tx_in(sig_ETXC_se), .tx_clk_outp(pad_ETXCp), .tx_clk_outm(pad_ETXCn), .iref(ibp_200u_E[1]),
							.vtop(vtop_EC), .vbot(vbot_EC), .ctl(ctl_EC), .ctlb(ctlb_EC));

<<<<<<< HEAD
   // Differential Outputs
   pad_WTXCpa  =  sig_WTXD_se,     pad_WTXCna = ~sig_WTXD_se,
   pad_WTXCpb  =  sig_WTXD_se,     pad_WTXCnb = ~sig_WTXD_se,
   pad_STXCpa  =  sig_STXD_se,     pad_STXCna = ~sig_STXD_se,
   pad_STXCpb  =  sig_STXD_se,     pad_STXCnb = ~sig_STXD_se,
   pad_ETXCpa  =  sig_ETXD_se,     pad_ETXCna = ~sig_ETXD_se,
   pad_ETXCpb  =  sig_ETXD_se,     pad_ETXCnb = ~sig_ETXD_se,
   pad_NTXCpa  =  sig_NTXD_se,     pad_NTXCna = ~sig_NTXD_se,
   pad_NTXCpb  =  sig_NTXD_se,     pad_NTXCnb = ~sig_NTXD_se,

   pad_WTXDpa  =  sig_WTXD_se,     pad_WTXDna = ~sig_WTXD_se,
   pad_WTXDpb  =  sig_WTXD_se,     pad_WTXDnb = ~sig_WTXD_se,
   pad_STXDpa  =  sig_STXD_se,     pad_STXDna = ~sig_STXD_se,
   pad_STXDpb  =  sig_STXD_se,     pad_STXDnb = ~sig_STXD_se,
   pad_ETXDpa  =  sig_ETXD_se,     pad_ETXDna = ~sig_ETXD_se,
   pad_ETXDpb  =  sig_ETXD_se,     pad_ETXDnb = ~sig_ETXD_se,
   pad_NTXDpa  =  sig_NTXD_se,     pad_NTXDna = ~sig_NTXD_se,
   pad_NTXDpb  =  sig_NTXD_se,     pad_NTXDnb = ~sig_NTXD_se,

   // Differential Inputs
   sig_WRXC_se =  pad_WRXCp  &&  !pad_WRXCn  ?  1'b1  :  !pad_WRXCp  &&  pad_WRXCn  ?  1'b0 : 1'bx,
   sig_WRXD_se =  pad_WRXDp  &&  !pad_WRXDn  ?  1'b1  :  !pad_WRXDp  &&  pad_WRXDn  ?  1'b0 : 1'bx,
   sig_SRXC_se =  pad_SRXCp  &&  !pad_SRXCn  ?  1'b1  :  !pad_SRXCp  &&  pad_SRXCn  ?  1'b0 : 1'bx,
   sig_SRXD_se =  pad_SRXDp  &&  !pad_SRXDn  ?  1'b1  :  !pad_SRXDp  &&  pad_SRXDn  ?  1'b0 : 1'bx,
   sig_ERXC_se =  pad_ERXCp  &&  !pad_ERXCn  ?  1'b1  :  !pad_ERXCp  &&  pad_ERXCn  ?  1'b0 : 1'bx,
   sig_ERXD_se =  pad_ERXDp  &&  !pad_ERXDn  ?  1'b1  :  !pad_ERXDp  &&  pad_ERXDn  ?  1'b0 : 1'bx,
   sig_NRXC_se =  pad_NRXCp  &&  !pad_NRXCn  ?  1'b1  :  !pad_NRXCp  &&  pad_NRXCn  ?  1'b0 : 1'bx,
   sig_NRXD_se =  pad_NRXDp  &&  !pad_NRXDn  ?  1'b1  :  !pad_NRXDp  &&  pad_NRXDn  ?  1'b0 : 1'bx,

	PDVSS_18_18_NT_DR_H dvss17 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVDD_18_18_NT_DR_H dvdd17 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	lvds_tx_top lvds_tx_ETXD (.vddd(vddcore), .vss(vss), .vddio(vddio), .rto(tielo_vss), .sns(tiehi_vddio),
	                        .enb(tx_pd_E), .tx_offset_trim(tx_offset_trim_E), .tx_bias_trim(tx_bias_trim_E), .tx_slew(tx_slew_E), 
							.tx_in(sig_ETXD_se), .tx_clk_outp(pad_ETXDp), .tx_clk_outm(pad_ETXDn), .iref(ibp_200u_E[0]),
							.vtop(vtop_ED), .vbot(vbot_ED), .ctl(ctl_ED), .ctlb(ctlb_ED));
	
	PDVSS_18_18_NT_DR_H dvss18 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVDD_18_18_NT_DR_H dvdd18 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_H  vss19  (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	PBIDIR_18_18_NT_DR_H gpio_10_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_gpio_10_do), .PAD(pad_gpio_10), .A(sig_gpio_10_di), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_10_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVSS_08_08_NT_DR_H  vss20 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVDD_08_08_NT_DR_H  vdd15 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_H  vss21 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	PBIDIR_18_18_NT_DR_H gpio_11_E (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_gpio_11_do), .PAD(pad_gpio_11), .A(sig_gpio_11_di), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_11_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	///////////////////////////
    ////////// North///////////
	///////////////////////////

	PBIDIR_18_18_NT_DR_V gpio_12_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_gpio_12_do), .PAD(pad_gpio_12), .A(sig_gpio_12_di), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_12_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVSS_08_08_NT_DR_V  vss22 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVDD_08_08_NT_DR_V  vdd16 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_V  vss23 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	PBIDIR_18_18_NT_DR_V gpio_13_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_gpio_13_do), .PAD(pad_gpio_13), .A(sig_gpio_13_di), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_13_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVDD_08_08_NT_DR_V  vdd17  (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_V  vss24  (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVDD_18_18_NT_DR_V dvdd19 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVSS_18_18_NT_DR_V dvss19 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	lvds_rx_top lvds_rx_NRXD (.vddc(vddcore), .vss(vss), .vddio(vddio), .rto(tielo_vss), .sns(tiehi_vddio),
	                        .pd_c(rx_pd_N), .trim_c(tielo_vss), .rx_cdac_p(rx_cdac_p_N), .rx_cdac_m(rx_cdac_m_N),
							.rx_bias_trim(rx_bias_trim_N), .rx_offset_trim(rx_offset_trim_N), .rx_gain_trim(), .rx_rterm_trim(rx_rterm_trim_N), 
							.rx_inp(pad_NRXDp), .rx_inm(pad_NRXDn), .rx_out(sig_NRXD_se), .rx_outb(), .ibn_200u(ibn_200u_N[0]), .ibp_50u(ibp_50u_N[0]),
							.analog_test(), .testbus_in_1(tb_rx1_1_N), .testbus_in_2(tb_rx1_2_N), .testbus_out_1(tb_rx1_1_N), .testbus_out_2(tb_rx1_2_N),
							.testbus_off(tb_off_N), .vref_tg_en(vref_tg_en_N), .vref_in(vref_tb_N),
							.ampout_p(), .ampout_m(), .compout_p(), .compout_m());
	
	PDVDD_18_18_NT_DR_V dvdd20 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVSS_18_18_NT_DR_V dvss20 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	lvds_rx_top lvds_rx_NRXC (.vddc(vddcore), .vss(vss), .vddio(vddio), .rto(tielo_vss), .sns(tiehi_vddio),
	                        .pd_c(rx_pd_N), .trim_c(tielo_vss), .rx_cdac_p(rx_cdac_p_N), .rx_cdac_m(rx_cdac_m_N),
							.rx_bias_trim(rx_bias_trim_N), .rx_offset_trim(rx_offset_trim_N), .rx_gain_trim(), .rx_rterm_trim(rx_rterm_trim_N), 
							.rx_inp(pad_NRXCp), .rx_inm(pad_NRXCn), .rx_out(sig_NRXC_se), .rx_outb(sig_NRXC_seb), .ibn_200u(ibn_200u_N[1]), .ibp_50u(ibp_50u_N[1]),
							.analog_test(), .testbus_in_1(tb_rx1_1_N), .testbus_in_2(tb_rx1_2_N), .testbus_out_1(tb_rx1_1_N), .testbus_out_2(tb_rx1_2_N),
							.testbus_off(tb_off_N), .vref_tg_en(vref_tg_en_N), .vref_in(vref_tb_N),
							.ampout_p(), .ampout_m(), .compout_p(), .compout_m());
	
	PDVDD_18_18_NT_DR_V dvdd21 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVSS_18_18_NT_DR_V dvss21 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVDD_08_08_NT_DR_V  vdd18  (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVDD_08_08_NT_DR_V  vdd19  (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_V  vss25  (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	ts_pmu  #(.PMU_START_DELAY(PMU_START_DELAY))     pmu_lvds_N (.vddd(vddcore), .vssio(vss), .vddio(vddio),
	                        .pd(pmu_pd_N), .pmu_bias_trim(pmu_bias_trim_N), .vref_tg_en(vref_tg_en_N), .testbus_off(tb_off_N),
							.vref(vref_N), .rref(rref_N), .vref_out(vref_tb_N), .ibp_200u(ibp_200u_N), .ibp_50u(ibp_50u_N), .ibn_200u(ibn_200u_N));
							
	
	PBIDIR_18_18_NT_DR_V jtag_tck_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_jtag_tck), .PAD(pad_jtag_tck), .A(), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(vss), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PBIDIR_18_18_NT_DR_V jtag_tdo_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(), .PAD(pad_jtag_tdo), .A(sig_jtag_tdo), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(vddcore), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PBIDIR_18_18_NT_DR_V jtag_tdi_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_jtag_tdi), .PAD(pad_jtag_tdi), .A(), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(vss), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PBIDIR_18_18_NT_DR_V jtag_tms_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_jtag_tms), .PAD(pad_jtag_tms), .A(), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(vss), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PBIDIR_18_18_NT_DR_V jtag_trst_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_jtag_trst), .PAD(pad_jtag_trst), .A(), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(vss), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PDVSS_18_18_NT_DR_V dvss22 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVDD_18_18_NT_DR_V dvdd22 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	lvds_tx_top lvds_tx_NTXC (.vddd(vddcore), .vss(vss), .vddio(vddio), .rto(tielo_vss), .sns(tiehi_vddio),
	                        .enb(tx_pd_N), .tx_offset_trim(tx_offset_trim_N), .tx_bias_trim(tx_bias_trim_N), .tx_slew(tx_slew_N), 
							.tx_in(sig_NTXC_se), .tx_clk_outp(pad_NTXCp), .tx_clk_outm(pad_NTXCn), .iref(ibp_200u_N[1]),
							.vtop(vtop_NC), .vbot(vbot_NC), .ctl(ctl_NC), .ctlb(ctlb_NC));


	PDVSS_18_18_NT_DR_V dvss23 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVDD_18_18_NT_DR_V dvdd23 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	lvds_tx_top lvds_tx_NTXD (.vddd(vddcore), .vss(vss), .vddio(vddio), .rto(tielo_vss), .sns(tiehi_vddio),
	                        .enb(tx_pd_N), .tx_offset_trim(tx_offset_trim_N), .tx_bias_trim(tx_bias_trim_N), .tx_slew(tx_slew_N), 
							.tx_in(sig_NTXD_se), .tx_clk_outp(pad_NTXDp), .tx_clk_outm(pad_NTXDn), .iref(ibp_200u_N[0]),
							.vtop(vtop_ND), .vbot(vbot_ND), .ctl(ctl_ND), .ctlb(ctlb_ND));
	
	PDVSS_18_18_NT_DR_V dvss24 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PDVDD_18_18_NT_DR_V dvdd24 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_V  vss26  (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	PBIDIR_18_18_NT_DR_V gpio_14_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_gpio_14_do), .PAD(pad_gpio_14), .A(sig_gpio_14_di), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_14_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));
	
	PVSS_08_08_NT_DR_V  vss27 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVDD_08_08_NT_DR_V  vdd20 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	PVSS_08_08_NT_DR_V  vss28 (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio));
	
	PBIDIR_18_18_NT_DR_V gpio_15_N (.VDD(vddcore), .VSS(vss), .DVDD(vddio), .DVSS(vss), .RTO(tielo_vss), .SNS(tiehi_vddio),
	                                 .PO(), .Y(sig_gpio_15_do), .PAD(pad_gpio_15), .A(sig_gpio_15_di), .DS0(tiehi_vddio), .DS1(tielo_vss), 
                                 .IE(tiehi_vddcore), .IS(tielo_vss), .OE(sig_gpio_15_oe), .PE(tielo_vss), .PS(tielo_vss), .POE(tielo_vss), .SR(tielo_vss));

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
