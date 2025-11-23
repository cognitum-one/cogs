//================================================================================================================================
//
//   Copyright (c) 2024 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name         :  NEWPORT
//
//   Description          :  Address Definitions for A2S I/O and Tile Accesses
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
//   Notes:
//
//
//================================================================================================================================

// Address Spaces
localparam Code                           = 32'h00000000;
localparam Data                           = 32'h40000000;
localparam Work                           = 32'h80000000;
localparam Regs                           = 32'hc0000000;

// I/O Module Base Addresses
localparam SHA256_base                    = 13'h1000;
localparam XSALSA_base                    = 13'h1008;
localparam GATEWAY_base                   = 13'h1010;
localparam TILE_base                      = 13'h1020;
localparam NEWS_base                      = 13'h1100;
localparam ROPUF_base                     = 13'h1200;
localparam AES_base                       = 13'h1300;
localparam GCM_base                       = 13'h1400;
localparam TMR0_base                      = 13'h1500;
localparam TMR1_base                      = 13'h1510;
localparam ZCKS_base                      = 13'h1520;
localparam JTAG_base                      = 13'h1530;
localparam PVT_base                       = 13'h1540;
localparam FLAG_base                      = 13'h1560;
localparam TIMO_base                      = 13'h1562;

// I/O Module Number of Registers 
localparam SHA256_range                   = 7;
localparam XSALSA_range                   = 5;
localparam GATEWAY_range                  = 12;
localparam TILE_range                     = 5;
localparam NEWS_range                     = 23;
localparam ROPUF_range                    = 64;
localparam AES_range                      = 193;
localparam GCM_range                      = 32;
localparam TMR0_range                     = 5;
localparam TMR1_range                     = 5;
localparam ZCKS_range                     = 4;
localparam JTAG_range                     = 3;
localparam PVT_range                      = 4;
localparam FLAG_range                     = 2;
localparam TIMO_range                     = 1;

// I/O module Register Indicies
localparam SHA256_MESG                    = 0;
localparam SHA256_HASH                    = 1;
localparam SHA256_STOP                    = 2;
localparam SHA256_STRT_RDY                = 3;
localparam SHA256_INIT                    = 4;
localparam SHA256_WAKE                    = 5;
localparam SHA256_SLEEP                   = 6;

localparam XSALSA_XBUF                    = 0;
localparam XSALSA_XBBS                    = 1;
localparam XSALSA_STOP                    = 2;
localparam XSALSA_STRT_RDY                = 3;
localparam XSALSA_CTRL                    = 4;

localparam GATEWAY_MyID                   = 0;
localparam GATEWAY_ADDR                   = 1;
localparam GATEWAY_DATA                   = 2;
localparam GATEWAY_CMND                   = 3;
localparam GATEWAY_BUSY                   = 4;
localparam GATEWAY_RDA1                   = 5;
localparam GATEWAY_RDA0                   = 6;
localparam GATEWAY_RESP                   = 7;
localparam GATEWAY_TIMO                   = 8;
localparam GATEWAY_TIMB                   = 9;
localparam GATEWAY_TIMR                   = 10;
localparam GATEWAY_FLAG                   = 11;

localparam TILE_RD_CLOCKS                 = 0;
localparam TILE_RD_DIVIDE                 = 1;
localparam TILE_MATCH_TAGS                = 2;
localparam TILE_SCRATCH                   = 3;
localparam TILE_PUF_OSCS                  = 4;

localparam NEWS_N_HEAD                    = 0;
localparam NEWS_E_HEAD                    = 1;
localparam NEWS_W_HEAD                    = 2;
localparam NEWS_S_HEAD                    = 3;
localparam NEWS_N_CSR                     = 4;
localparam NEWS_E_CSR                     = 5;
localparam NEWS_W_CSR                     = 6;
localparam NEWS_S_CSR                     = 7;
localparam NEWS_L_CSR                     = 8;
localparam NEWS_L_IN                      = 9;
localparam NEWS_L_EOF                     = 10;
localparam NEWS_N_CONTROL                 = 11;
localparam NEWS_E_CONTROL                 = 12;
localparam NEWS_W_CONTROL                 = 13;
localparam NEWS_S_CONTROL                 = 14;
localparam NEWS_L_OUT                     = 15;
localparam NEWS_N_OUTPUT                  = 16;
localparam NEWS_E_OUTPUT                  = 17;
localparam NEWS_W_OUTPUT                  = 18;
localparam NEWS_S_OUTPUT                  = 19;
localparam NEWS_L_DMA_INCOMING            = 20;
localparam NEWS_L_DMA_OUTGOING            = 21;
localparam NEWS_L_DMA_STATE               = 22;

localparam ROPUF_WR_CONTROL               = 0;
localparam ROPUF_WR_NUM_CHAL              = 1;
localparam ROPUF_WR_S_INC                 = 2;
localparam ROPUF_WR_MAX_CNT               = 3;
localparam ROPUF_WR_SEL1                  = 4;
localparam ROPUF_WR_SEL2                  = 5;
localparam ROPUF_WR_ECC                   = 6;
localparam ROPUF_WR_HD                    = 7;
localparam ROPUF_WR_ECC_RATE              = 8;
localparam ROPUF_WR_KEY_ID                = 9;
localparam ROPUF_WR_INPUT_KEY0            = 10;
localparam ROPUF_WR_INPUT_KEY1            = 11;
localparam ROPUF_WR_INPUT_KEY2            = 12;
localparam ROPUF_WR_INPUT_KEY3            = 13;
localparam ROPUF_WR_ENCRYPTED_KEY0        = 14;
localparam ROPUF_WR_ENCRYPTED_KEY1        = 15;
localparam ROPUF_WR_ENCRYPTED_KEY2        = 16;
localparam ROPUF_WR_ENCRYPTED_KEY3        = 17;
localparam ROPUF_WR_CORE_EN_KEY0          = 18;
localparam ROPUF_WR_CORE_EN_KEY1          = 19;
localparam ROPUF_WR_CORE_EN_KEY2          = 20;
localparam ROPUF_WR_CORE_EN_KEY3          = 21;
localparam ROPUF_WR_DISCOVERY_SEED0       = 22;
localparam ROPUF_WR_DISCOVERY_SEED1       = 23;
localparam ROPUF_WR_DISCOVERY_SEED2       = 24;
localparam ROPUF_WR_DISCOVERY_SEED3       = 25;
localparam ROPUF_WR_DEVICE_ID0            = 26;
localparam ROPUF_WR_DEVICE_ID1            = 27;
localparam ROPUF_WR_DEVICE_ID2            = 28;
localparam ROPUF_WR_DEVICE_ID3            = 29;
localparam ROPUF_WR_IV_DIVERSIFIER0       = 30;
localparam ROPUF_WR_IV_DIVERSIFIER1       = 31;
localparam ROPUF_WR_IV_DIVERSIFIER2       = 32;
localparam ROPUF_WR_IV_DIVERSIFIER3       = 33;
localparam ROPUF_WR_KEY_DIVERSIFIER0      = 34;
localparam ROPUF_WR_KEY_DIVERSIFIER1      = 35;
localparam ROPUF_WR_KEY_DIVERSIFIER2      = 36;
localparam ROPUF_WR_KEY_DIVERSIFIER3      = 37;
localparam ROPUF_WR_IRQ_EN                = 38;
localparam ROPUF_WR_KEY_MANAGER_CTRL      = 39;
localparam ROPUF_WR_TRNG_CTRL             = 40;
localparam ROPUF_RD_STATUS                = 41;
localparam ROPUF_RD_ECC                   = 42;
localparam ROPUF_RD_ID                    = 43;
localparam ROPUF_RD_HD                    = 44;
localparam ROPUF_RD_TRNG                  = 45;
localparam ROPUF_RD_TRNG_STATUS           = 46;
localparam ROPUF_RD_KEY_MANAGER_STATUS    = 47;
localparam ROPUF_RD_SECURE_KEY0           = 48;
localparam ROPUF_RD_SECURE_KEY1           = 49;
localparam ROPUF_RD_SECURE_KEY2           = 50;
localparam ROPUF_RD_SECURE_KEY3           = 51;
localparam ROPUF_RD_DISCOVERY_RESP0       = 52;
localparam ROPUF_RD_DISCOVERY_RESP1       = 53;
localparam ROPUF_RD_DISCOVERY_RESP2       = 54;
localparam ROPUF_RD_DISCOVERY_RESP3       = 55;
localparam ROPUF_RD_ENCRYPTED_ID0         = 56;
localparam ROPUF_RD_ENCRYPTED_ID1         = 57;
localparam ROPUF_RD_ENCRYPTED_ID2         = 58;
localparam ROPUF_RD_ENCRYPTED_ID3         = 59;
localparam ROPUF_RW_spare1                = 60;
localparam ROPUF_RW_spare2                = 61;
localparam ROPUF_RW_spare3                = 62;
localparam ROPUF_RW_SYSTEM                = 63;

localparam AES_ISKTON_0                   = 0;
localparam AES_ISKTON_1                   = 1;
localparam AES_ISKTON_2                   = 2;
localparam AES_ISKTON_3                   = 3;
localparam AES_ISKTOS_0                   = 4;
localparam AES_ISKTOS_1                   = 5;
localparam AES_ISKTOS_2                   = 6;
localparam AES_ISKTOS_3                   = 7;
localparam AES_ISKTOE_0                   = 8;
localparam AES_ISKTOE_1                   = 9;
localparam AES_ISKTOE_2                   = 10;
localparam AES_ISKTOE_3                   = 11;
localparam AES_ISKTOW_0                   = 12;
localparam AES_ISKTOW_1                   = 13;
localparam AES_ISKTOW_2                   = 14;
localparam AES_ISKTOW_3                   = 15;
localparam AES_ISKFRN_0                   = 16;
localparam AES_ISKFRN_1                   = 17;
localparam AES_ISKFRN_2                   = 18;
localparam AES_ISKFRN_3                   = 19;
localparam AES_ISKFRS_0                   = 20;
localparam AES_ISKFRS_1                   = 21;
localparam AES_ISKFRS_2                   = 22;
localparam AES_ISKFRS_3                   = 23;
localparam AES_ISKFRE_0                   = 24;
localparam AES_ISKFRE_1                   = 25;
localparam AES_ISKFRE_2                   = 26;
localparam AES_ISKFRE_3                   = 27;
localparam AES_ISKFRW_0                   = 28;
localparam AES_ISKFRW_1                   = 29;
localparam AES_ISKFRW_2                   = 30;
localparam AES_ISKFRW_3                   = 31;
localparam AES_ISK0FR_0                   = 32;
localparam AES_ISK0FR_1                   = 33;
localparam AES_ISK0FR_2                   = 34;
localparam AES_ISK0FR_3                   = 35;
localparam AES_ISK0TO_0                   = 36;
localparam AES_ISK0TO_1                   = 37;
localparam AES_ISK0TO_2                   = 38;
localparam AES_ISK0TO_3                   = 39;
localparam AES_ISK1FR_0                   = 40;
localparam AES_ISK1FR_1                   = 41;
localparam AES_ISK1FR_2                   = 42;
localparam AES_ISK1FR_3                   = 43;
localparam AES_ISK1TO_0                   = 44;
localparam AES_ISK1TO_1                   = 45;
localparam AES_ISK1TO_2                   = 46;
localparam AES_ISK1TO_3                   = 47;
localparam AES_ISKMLT_0                   = 48;
localparam AES_ISKMLT_1                   = 49;
localparam AES_ISKMLT_2                   = 50;
localparam AES_ISKMLT_3                   = 51;
localparam AES_ISKSP0_0                   = 52;
localparam AES_ISKSP0_1                   = 53;
localparam AES_ISKSP0_2                   = 54;
localparam AES_ISKSP0_3                   = 55;
localparam AES_ISKSP1_0                   = 56;
localparam AES_ISKSP1_1                   = 57;
localparam AES_ISKSP1_2                   = 58;
localparam AES_ISKSP1_3                   = 59;
localparam AES_ISKSP2_0                   = 60;
localparam AES_ISKSP2_1                   = 61;
localparam AES_ISKSP2_2                   = 62;
localparam AES_ISKSP2_3                   = 63;
localparam AES_IVCTON_0                   = 64;
localparam AES_IVCTON_1                   = 65;
localparam AES_IVCTON_2                   = 66;
localparam AES_IVCTON_3                   = 67;
localparam AES_IVCTOS_0                   = 68;
localparam AES_IVCTOS_1                   = 69;
localparam AES_IVCTOS_2                   = 70;
localparam AES_IVCTOS_3                   = 71;
localparam AES_IVCTOE_0                   = 72;
localparam AES_IVCTOE_1                   = 73;
localparam AES_IVCTOE_2                   = 74;
localparam AES_IVCTOE_3                   = 75;
localparam AES_IVCTOW_0                   = 76;
localparam AES_IVCTOW_1                   = 77;
localparam AES_IVCTOW_2                   = 78;
localparam AES_IVCTOW_3                   = 79;
localparam AES_IVCFRN_0                   = 80;
localparam AES_IVCFRN_1                   = 81;
localparam AES_IVCFRN_2                   = 82;
localparam AES_IVCFRN_3                   = 83;
localparam AES_IVCFRS_0                   = 84;
localparam AES_IVCFRS_1                   = 85;
localparam AES_IVCFRS_2                   = 86;
localparam AES_IVCFRS_3                   = 87;
localparam AES_IVCFRE_0                   = 88;
localparam AES_IVCFRE_1                   = 89;
localparam AES_IVCFRE_2                   = 90;
localparam AES_IVCFRE_3                   = 91;
localparam AES_IVCFRW_0                   = 92;
localparam AES_IVCFRW_1                   = 93;
localparam AES_IVCFRW_2                   = 94;
localparam AES_IVCFRW_3                   = 95;
localparam AES_IVC0FR_0                   = 96;
localparam AES_IVC0FR_1                   = 97;
localparam AES_IVC0FR_2                   = 98;
localparam AES_IVC0FR_3                   = 99;
localparam AES_IVC0TO_0                   = 100;
localparam AES_IVC0TO_1                   = 101;
localparam AES_IVC0TO_2                   = 102;
localparam AES_IVC0TO_3                   = 103;
localparam AES_IVC1FR_0                   = 104;
localparam AES_IVC1FR_1                   = 105;
localparam AES_IVC1FR_2                   = 106;
localparam AES_IVC1FR_3                   = 107;
localparam AES_IVC1TO_0                   = 108;
localparam AES_IVC1TO_1                   = 109;
localparam AES_IVC1TO_2                   = 110;
localparam AES_IVC1TO_3                   = 111;
localparam AES_IVCMLT_0                   = 112;
localparam AES_IVCMLT_1                   = 113;
localparam AES_IVCMLT_2                   = 114;
localparam AES_IVCMLT_3                   = 115;
localparam AES_IVCSP0_0                   = 116;
localparam AES_IVCSP0_1                   = 117;
localparam AES_IVCSP0_2                   = 118;
localparam AES_IVCSP0_3                   = 119;
localparam AES_IVCSP1_0                   = 120;
localparam AES_IVCSP1_1                   = 121;
localparam AES_IVCSP1_2                   = 122;
localparam AES_IVCSP1_3                   = 123;
localparam AES_IVCSP2_0                   = 124;
localparam AES_IVCSP2_1                   = 125;
localparam AES_IVCSP2_2                   = 126;
localparam AES_IVCSP2_3                   = 127;
localparam AES_ISKTNC_0                   = 128;
localparam AES_ISKTNC_1                   = 129;
localparam AES_ISKTNC_2                   = 130;
localparam AES_ISKTNC_3                   = 131;
localparam AES_ISKTSC_0                   = 132;
localparam AES_ISKTSC_1                   = 133;
localparam AES_ISKTSC_2                   = 134;
localparam AES_ISKTSC_3                   = 135;
localparam AES_ISKTEC_0                   = 136;
localparam AES_ISKTEC_1                   = 137;
localparam AES_ISKTEC_2                   = 138;
localparam AES_ISKTEC_3                   = 139;
localparam AES_ISKTWC_0                   = 140;
localparam AES_ISKTWC_1                   = 141;
localparam AES_ISKTWC_2                   = 142;
localparam AES_ISKTWC_3                   = 143;
localparam AES_ISKFNC_0                   = 144;
localparam AES_ISKFNC_1                   = 145;
localparam AES_ISKFNC_2                   = 146;
localparam AES_ISKFNC_3                   = 147;
localparam AES_ISKFSC_0                   = 148;
localparam AES_ISKFSC_1                   = 149;
localparam AES_ISKFSC_2                   = 150;
localparam AES_ISKFSC_3                   = 151;
localparam AES_ISKFEC_0                   = 152;
localparam AES_ISKFEC_1                   = 153;
localparam AES_ISKFEC_2                   = 154;
localparam AES_ISKFEC_3                   = 155;
localparam AES_ISKFWC_0                   = 156;
localparam AES_ISKFWC_1                   = 157;
localparam AES_ISKFWC_2                   = 158;
localparam AES_ISKFWC_3                   = 159;
localparam AES_ISK0FC_0                   = 160;
localparam AES_ISK0FC_1                   = 161;
localparam AES_ISK0FC_2                   = 162;
localparam AES_ISK0FC_3                   = 163;
localparam AES_ISK0TC_0                   = 164;
localparam AES_ISK0TC_1                   = 165;
localparam AES_ISK0TC_2                   = 166;
localparam AES_ISK0TC_3                   = 167;
localparam AES_ISK1FC_0                   = 168;
localparam AES_ISK1FC_1                   = 169;
localparam AES_ISK1FC_2                   = 170;
localparam AES_ISK1FC_3                   = 171;
localparam AES_ISK1TC_0                   = 172;
localparam AES_ISK1TC_1                   = 173;
localparam AES_ISK1TC_2                   = 174;
localparam AES_ISK1TC_3                   = 175;
localparam AES_ISKMLC_0                   = 176;
localparam AES_ISKMLC_1                   = 177;
localparam AES_ISKMLC_2                   = 178;
localparam AES_ISKMLC_3                   = 179;
localparam AES_ISKS0C_0                   = 180;
localparam AES_ISKS0C_1                   = 181;
localparam AES_ISKS0C_2                   = 182;
localparam AES_ISKS0C_3                   = 183;
localparam AES_ISKS1C_0                   = 184;
localparam AES_ISKS1C_1                   = 185;
localparam AES_ISKS1C_2                   = 186;
localparam AES_ISKS1C_3                   = 187;
localparam AES_ISKS2C_0                   = 188;
localparam AES_ISKS2C_1                   = 189;
localparam AES_ISKS2C_2                   = 190;
localparam AES_ISKS2C_3                   = 191;
localparam AES_CSR                        = 192;

localparam GCM_S0                         = 0;
localparam GCM_S1                         = 1;
localparam GCM_S2                         = 2;
localparam GCM_S3                         = 3;
localparam GCM_T0                         = 4;
localparam GCM_T1                         = 5;
localparam GCM_T2                         = 6;
localparam GCM_T3                         = 7;
localparam GCM_U0                         = 8;
localparam GCM_U1                         = 9;
localparam GCM_U2                         = 10;
localparam GCM_U3                         = 11;
localparam GCM_V0                         = 12;
localparam GCM_V1                         = 13;
localparam GCM_V2                         = 14;
localparam GCM_V3                         = 15;
localparam GCM_W0                         = 16;
localparam GCM_W1                         = 17;
localparam GCM_W2                         = 18;
localparam GCM_W3                         = 19;
localparam GCM_X0                         = 20;
localparam GCM_X1                         = 21;
localparam GCM_X2                         = 22;
localparam GCM_X3                         = 23;
localparam GCM_Y0                         = 24;
localparam GCM_Y1                         = 25;
localparam GCM_Y2                         = 26;
localparam GCM_Y3                         = 27;
localparam GCM_Z0                         = 28;
localparam GCM_Z1                         = 29;
localparam GCM_Z2                         = 30;
localparam GCM_Z3                         = 31;

localparam TMR0_CSR                       = 0;
localparam TMR0_VALUE                     = 1;
localparam TMR0_SCALE                     = 2;
localparam TMR0_LOWER                     = 3;
localparam TMR0_UPPER                     = 4;

localparam TMR1_CSR                       = 0;
localparam TMR1_VALUE                     = 1;
localparam TMR1_SCALE                     = 2;
localparam TMR1_LOWER                     = 3;
localparam TMR1_UPPER                     = 4;

localparam ZCKS_TILE_CTRL                 = 0;
localparam ZCKS_RACE_CTRL                 = 1;
localparam ZCKS_NEWS_CTRL                 = 2;
localparam ZCKS_NEWS_DIVD                 = 3;

localparam JTAG_IRDY                      = 0;
localparam JTAG_ORDY                      = 1;
localparam JTAG_DATA                      = 2;

localparam PVT_FREQ                       = 0;
localparam PVT_CMND                       = 1;
localparam PVT_STATUS                     = 2;
localparam PVT_DATA                       = 3;

localparam FLAG_IN                        = 0;
localparam FLAG_OUT                       = 1;

localparam TIMO_IN                        = 0;

// Raceway Addresses
localparam aSYS_TILE_SET_CLOCKS           = 32'hffff8000;
localparam aSYS_TILE_SET_DIVIDE           = 32'hffff8004;
localparam aSYS_TILE_A2S_RESET            = 32'hffff8008;
