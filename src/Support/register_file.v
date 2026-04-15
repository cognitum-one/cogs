//--------------------------------------------------------------------------------
//  (C) COPYRIGHT 2020-2023 Low Power Futures, Inc.  ALL RIGHTS RESERVED
//--------------------------------------------------------------------------------
// Company: Low Power Futures, Inc.
// Engineer: aberthe.
//
// Create Date: 02/14/2022 07:31:16 PM
// Design Name:
// Module Name: apb_interface - behavioral
// Project Name:
// Revision:
// Revision 0.01 - File Created
// Additional Comments:
//
//--------------------------------------------------------------------------------
// no timescale needed

`ifndef _register_file_
`define _register_file_

`define RO_PUF_REG_CNT                    60
`define RO_PUF_READ_REG_CNT               19
`define RO_PUF_WRITE_REG_CNT              41
`define RO_PUF_REG_MEM_LOW                0
`define RO_PUF_REG_MEM_HIGH               236
`define RO_PUF_REG_WIDTH                  32

`define WR_RO_PUF_CONTROL_ADDR            0
`define WR_RO_PUF_NUM_CHAL_ADDR           1
`define WR_RO_PUF_S_INC_ADDR              2
`define WR_RO_PUF_MAX_CNT_ADDR            3
`define WR_RO_PUF_SEL1_ADDR               4
`define WR_RO_PUF_SEL2_ADDR               5
`define WR_RO_PUF_ECC_ADDR                6
`define WR_RO_PUF_HD_ADDR                 7
`define WR_RO_PUF_ECC_RATE_ADDR           8
`define WR_RO_PUF_KEY_ID_ADDR             9

`define WR_RO_PUF_INPUT_KEY0_ADDR         10
`define WR_RO_PUF_INPUT_KEY1_ADDR         11
`define WR_RO_PUF_INPUT_KEY2_ADDR         12
`define WR_RO_PUF_INPUT_KEY3_ADDR         13

`define WR_RO_PUF_ENCRYPTED_KEY0_ADDR     14
`define WR_RO_PUF_ENCRYPTED_KEY1_ADDR     15
`define WR_RO_PUF_ENCRYPTED_KEY2_ADDR     16
`define WR_RO_PUF_ENCRYPTED_KEY3_ADDR     17

`define WR_RO_PUF_CORE_EN_KEY0_ADDR       18
`define WR_RO_PUF_CORE_EN_KEY1_ADDR       19
`define WR_RO_PUF_CORE_EN_KEY2_ADDR       20
`define WR_RO_PUF_CORE_EN_KEY3_ADDR       21

`define WR_RO_PUF_DISCOVERY_SEED0_ADDR    22
`define WR_RO_PUF_DISCOVERY_SEED1_ADDR    23
`define WR_RO_PUF_DISCOVERY_SEED2_ADDR    24
`define WR_RO_PUF_DISCOVERY_SEED3_ADDR    25

`define WR_RO_PUF_DEVICE_ID0_ADDR         26
`define WR_RO_PUF_DEVICE_ID1_ADDR         27
`define WR_RO_PUF_DEVICE_ID2_ADDR         28
`define WR_RO_PUF_DEVICE_ID3_ADDR         29

`define WR_RO_PUF_IV_DIVERSIFIER0_ADDR    30
`define WR_RO_PUF_IV_DIVERSIFIER1_ADDR    31
`define WR_RO_PUF_IV_DIVERSIFIER2_ADDR    32
`define WR_RO_PUF_IV_DIVERSIFIER3_ADDR    33

`define WR_RO_PUF_KEY_DIVERSIFIER0_ADDR   34
`define WR_RO_PUF_KEY_DIVERSIFIER1_ADDR   35
`define WR_RO_PUF_KEY_DIVERSIFIER2_ADDR   36
`define WR_RO_PUF_KEY_DIVERSIFIER3_ADDR   37

`define WR_RO_PUF_IRQ_EN_ADDR             38
`define WR_RO_PUF_KEY_MNGER_CTRL_ADDR     39
`define WR_RO_PUF_TRNG_CTRL_ADDR          40

`define RD_RO_PUF_STATUS_ADDR             41
`define RD_RO_PUF_ECC_ADDR                42
`define RD_RO_PUF_ID_ADDR                 43
`define RD_RO_PUF_HD_ADDR                 44
`define RD_RO_PUF_TRNG_ADDR               45
`define RD_RO_PUF_TRNG_STATUS_ADDR        46
`define RD_RO_PUF_KEY_MNGER_STATUS_ADDR   47

`define RD_RO_PUF_SECURE_KEY0_ADDR        48
`define RD_RO_PUF_SECURE_KEY1_ADDR        49
`define RD_RO_PUF_SECURE_KEY2_ADDR        50
`define RD_RO_PUF_SECURE_KEY3_ADDR        51

`define RD_RO_PUF_DISCOVERY_RESP0_ADDR    52
`define RD_RO_PUF_DISCOVERY_RESP1_ADDR    53
`define RD_RO_PUF_DISCOVERY_RESP2_ADDR    54
`define RD_RO_PUF_DISCOVERY_RESP3_ADDR    55

`define RD_RO_PUF_ENCRYPTED_ID0_ADDR      56
`define RD_RO_PUF_ENCRYPTED_ID1_ADDR      57
`define RD_RO_PUF_ENCRYPTED_ID2_ADDR      58
`define RD_RO_PUF_ENCRYPTED_ID3_ADDR      59

`define WR_RO_PUF_CONTROL_REG_ID          0
`define WR_RO_PUF_NUM_CHAL_REG_ID         1
`define WR_RO_PUF_S_INC_REG_ID            2
`define WR_RO_PUF_MAX_CNT_REG_ID          3
`define WR_RO_PUF_SEL1_REG_ID             4
`define WR_RO_PUF_SEL2_REG_ID             5
`define WR_RO_PUF_ECC_REG_ID              6
`define WR_RO_PUF_HD_REG_ID               7
`define WR_RO_PUF_ECC_RATE_REG_ID         8
`define WR_RO_PUF_KEY_ID_REG_ID           9

`define WR_RO_PUF_INPUT_KEY0_REG_ID       10
`define WR_RO_PUF_INPUT_KEY1_REG_ID       11
`define WR_RO_PUF_INPUT_KEY2_REG_ID       12
`define WR_RO_PUF_INPUT_KEY3_REG_ID       13

`define WR_RO_PUF_ENCRYPTED_KEY0_REG_ID   14
`define WR_RO_PUF_ENCRYPTED_KEY1_REG_ID   15
`define WR_RO_PUF_ENCRYPTED_KEY2_REG_ID   16
`define WR_RO_PUF_ENCRYPTED_KEY3_REG_ID   17

`define WR_RO_PUF_CORE_EN_KEY0_REG_ID     18
`define WR_RO_PUF_CORE_EN_KEY1_REG_ID     19
`define WR_RO_PUF_CORE_EN_KEY2_REG_ID     20
`define WR_RO_PUF_CORE_EN_KEY3_REG_ID     21

`define WR_RO_PUF_DISCOVERY_SEED0_REG_ID  22
`define WR_RO_PUF_DISCOVERY_SEED1_REG_ID  23
`define WR_RO_PUF_DISCOVERY_SEED2_REG_ID  24
`define WR_RO_PUF_DISCOVERY_SEED3_REG_ID  25

`define WR_RO_PUF_DEVICE_ID0_REG_ID       26
`define WR_RO_PUF_DEVICE_ID1_REG_ID       27
`define WR_RO_PUF_DEVICE_ID2_REG_ID       28
`define WR_RO_PUF_DEVICE_ID3_REG_ID       29

`define WR_RO_PUF_IV_DIVERSIFIER0_REG_ID  30
`define WR_RO_PUF_IV_DIVERSIFIER1_REG_ID  31
`define WR_RO_PUF_IV_DIVERSIFIER2_REG_ID  32
`define WR_RO_PUF_IV_DIVERSIFIER3_REG_ID  33

`define WR_RO_PUF_KEY_DIVERSIFIER0_REG_ID 34
`define WR_RO_PUF_KEY_DIVERSIFIER1_REG_ID 35
`define WR_RO_PUF_KEY_DIVERSIFIER2_REG_ID 36
`define WR_RO_PUF_KEY_DIVERSIFIER3_REG_ID 37

`define WR_RO_PUF_IRQ_EN_REG_ID           38
`define WR_RO_PUF_KEY_MNGER_CTRL_REG_ID   39
`define WR_RO_PUF_TRNG_CTRL_REG_ID        40

`define RD_RO_PUF_STATUS_REG_ID           0
`define RD_RO_PUF_ECC_REG_ID              1
`define RD_RO_PUF_ID_REG_ID               2
`define RD_RO_PUF_HD_REG_ID               3
`define RD_RO_PUF_TRNG_REG_ID             4
`define RD_RO_PUF_TRNG_STATUS_REG_ID      5
`define RD_RO_PUF_KEY_MNGER_STATUS_REG_ID 6

`define RD_RO_PUF_SECURE_KEY0_REG_ID      7
`define RD_RO_PUF_SECURE_KEY1_REG_ID      8
`define RD_RO_PUF_SECURE_KEY2_REG_ID      9
`define RD_RO_PUF_SECURE_KEY3_REG_ID      10

`define RD_RO_PUF_DISCOVERY_RESP0_REG_ID  11
`define RD_RO_PUF_DISCOVERY_RESP1_REG_ID  12
`define RD_RO_PUF_DISCOVERY_RESP2_REG_ID  13
`define RD_RO_PUF_DISCOVERY_RESP3_REG_ID  14

`define RD_RO_PUF_ENCRYPTED_ID0_REG_ID    15
`define RD_RO_PUF_ENCRYPTED_ID1_REG_ID    16
`define RD_RO_PUF_ENCRYPTED_ID2_REG_ID    17
`define RD_RO_PUF_ENCRYPTED_ID3_REG_ID    18


`define WR_RO_PUF_CONTROL_RST         32'h00000001
`define WR_RO_PUF_NUM_CHAL_RST        32'h00000000
`define WR_RO_PUF_S_INC_RST           32'h00000000
`define WR_RO_PUF_MAX_CNT_RST         32'h00000000
`define WR_RO_PUF_SEL1_RST            32'h00000000
`define WR_RO_PUF_SEL2_RST            32'h00000000
`define WR_RO_PUF_ECC_RST             32'h00000000
`define WR_RO_PUF_HD_RST              32'h00000000
`define WR_RO_PUF_ECC_RATE_RST        32'h00000000
`define WR_RO_PUF_KEY_ID_RST          32'h00000000
`define WR_RO_PUF_INPUT_KEY0_RST      32'h00000000
`define WR_RO_PUF_INPUT_KEY1_RST      32'h00000000
`define WR_RO_PUF_INPUT_KEY2_RST      32'h00000000
`define WR_RO_PUF_INPUT_KEY3_RST      32'h00000000
`define WR_RO_PUF_CORE_EN_KEY0_RST    32'h00000000
`define WR_RO_PUF_CORE_EN_KEY1_RST    32'h00000000
`define WR_RO_PUF_CORE_EN_KEY2_RST    32'h00000000
`define WR_RO_PUF_CORE_EN_KEY3_RST    32'h00000000
`define WR_RO_PUF_IRQ_EN_RST          32'h00000000

`define RD_RO_PUF_STATUS_RST          32'h00000000
`define RD_RO_PUF_ECC_RST             32'h00000000
`define RD_RO_PUF_ID_RST              32'h00000000
`define RD_RO_PUF_HD_RST              32'h00000000
`define RD_RO_PUF_SECURE_KEY0_RST     32'h00000000
`define RD_RO_PUF_SECURE_KEY1_RST     32'h00000000
`define RD_RO_PUF_SECURE_KEY2_RST     32'h00000000
`define RD_RO_PUF_SECURE_KEY3_RST     32'h00000000

`define PACK_REG_ARRAY(REG_WIDTH, LEN, DEST, SRC) \
genvar i; \
generate for (i = 0; i < LEN; i = i + 1) \
    begin \
        assign DEST[i][(REG_WIDTH-1):0] = SRC[(REG_WIDTH*i) + (REG_WIDTH-1):(REG_WIDTH*i)]; \
    end \
endgenerate

`define UNPACK_REG_ARRAY(REG_WIDTH, LEN, SRC, DEST) \
genvar j; \
generate for (j = 0; j < LEN; j = j + 1) \
    begin \
        assign DEST[(REG_WIDTH*j) + (REG_WIDTH-1):(REG_WIDTH*j)] = SRC[j][REG_WIDTH-1:0]; \
    end \
endgenerate

`endif
