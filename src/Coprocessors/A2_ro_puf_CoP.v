/* *******************************************************************************************************************************

   Copyright © 2024 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : CRYPTO Library

   Description          : RO PUF Wrapper

*********************************************************************************************************************************
   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*********************************************************************************************************************************
 NOTA BENE:

   Read-only regs are loaded as the incoming access arrives so they may be returned in the second cycle
   Read-write regs are loaded in the second cycle, The write data being captured are the end of the first cycle

********************************************************************************************************************************/

`include "A2_project_settings.vh"

module   A2_ro_puf_CoP #(
   parameter    [12:0]  BASE        = 13'h00000,   // Must be on modulo 32 boundary (<32 Addresses total)
   )(
   // CoP interface
   output wire  [32:0]  imiso,   // Slave data out {               IOk, IOo[31:0]}
   input  wire  [47:0]  imosi,   // Slave data in  {cIO[2:0] aIO[12:0], iIO[31:0]}
   output wire          flag,    // Condition Code flag Finished\Ready
   // IP interconnect
   output wire [xxx:0]  if2rp,   // Signals from this interface (if)  to  RO PUF (rp)
   input  wire [yyy:0]  rp2if,   // Signals  to  this interface (if) from RO PUF (rp)
   // Global
   input                reset,
   input                clock
   );

localparam
      WR_RO_PUF_CONTROL_ADDR        = 0,
      WR_RO_PUF_NUM_CHAL_ADDR       = 1,
      WR_RO_PUF_S_INC_ADDR          = 2,
      WR_RO_PUF_MAX_CNT_ADDR        = 3,
      WR_RO_PUF_SEL1_ADDR           = 4,
      WR_RO_PUF_SEL2_ADDR           = 5,
      WR_RO_PUF_ECC_ADDR            = 6,
      WR_RO_PUF_HD_ADDR             = 7,
      WR_RO_PUF_ECC_RATE_ADDR       = 8,
      WR_RO_PUF_KEY_ID_ADDR         = 9,
      WR_RO_PUF_INPUT_KEY0_ADDR     = 10,
      WR_RO_PUF_INPUT_KEY1_ADDR     = 11,
      WR_RO_PUF_INPUT_KEY2_ADDR     = 12,
      WR_RO_PUF_INPUT_KEY3_ADDR     = 13,
      WR_RO_PUF_CORE_EN_KEY0_ADDR   = 14,
      WR_RO_PUF_CORE_EN_KEY1_ADDR   = 15,
      WR_RO_PUF_CORE_EN_KEY2_ADDR   = 16,
      WR_RO_PUF_CORE_EN_KEY3_ADDR   = 17,
      WR_RO_PUF_IRQ_EN_ADDR         = 18,

      RD_RO_PUF_STATUS_ADDR         = 19,
      RD_RO_PUF_ECC_ADDR            = 20,
      RD_RO_PUF_ID_ADDR             = 21,
      RD_RO_PUF_HD_ADDR             = 22,
      RD_RO_PUF_SECURE_KEY0_ADDR    = 23,
      RD_RO_PUF_SECURE_KEY1_ADDR    = 24,
      RD_RO_PUF_SECURE_KEY2_ADDR    = 25,
      RD_RO_PUF_SECURE_KEY3_ADDR    = 26;

// Physical Registers ===========================================================================================================
reg   [31:0]   WR_RO_PUF_CONTROL,
               WR_RO_PUF_NUM_CHAL,
               WR_RO_PUF_S_INC,
               WR_RO_PUF_MAX_CNT,
               WR_RO_PUF_SEL1,
               WR_RO_PUF_SEL2,
               WR_RO_PUF_ECC,
               WR_RO_PUF_HD,
               WR_RO_PUF_ECC_RATE,
               WR_RO_PUF_KEY_ID,
               WR_RO_PUF_INPUT_KEY0,
               WR_RO_PUF_INPUT_KEY1,
               WR_RO_PUF_INPUT_KEY2,
               WR_RO_PUF_INPUT_KEY3,
               WR_RO_PUF_CORE_EN_KEY0,
               WR_RO_PUF_CORE_EN_KEY1,
               WR_RO_PUF_CORE_EN_KEY2,
               WR_RO_PUF_CORE_EN_KEY3,
               WR_RO_PUF_IRQ_EN;

// Decollate imosi ==============================================================================================================

wire  [ 2:0]   cIO;
wire  [12:0]   aIO;
wire  [31:0]   iIO;

assign {cIO,aIO,iIO} = imosi;

// Capture Access ===============================================================================================================

reg   [31:0]   IN;
reg            Wr, Rd;

wire  wr_base_hit    =  aIO[12:5] == BASE[12:5] & aIO[4:0] <= 18;
wire  rd_base_hit    =  aIO[12:5] == BASE[12:5] & aIO[4:0] <= 26;

always @(posedge clock)
   if ((cIO==3'b110) && wr_base_hit) IN[31:0] <= `tCQ iIO[31:0];

wire  reg_write   =  (cIO == 3'b110) & wr_base_hit;
wire  reg_read    =  (cIO == 3'b101) & rd_base_hit;

always @(posedge clock)
   if      (reset)            Wr <= `tCQ  1'b0;
   else if (Wr ^ reg_write)   Wr <= `tCQ  1'b1;

always @(posedge clock)
   if      (reset)            Rd <= `tCQ  1'b0;
   else if (Rd ^ reg_read)    Rd <= `tCQ  1'b1;

// Decode Addresses =============================================================================================================

wire  [26:0]   decode   = cIO[2] << aIO[4:0];

// Write to Registers ===========================================================================================================

always @(posedge clock) begin
   if (Wr && decode[WR_RO_PUF_CONTROL_ADDR     ])  WR_RO_PUF_CONTROL      <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_NUM_CHAL_ADDR    ])  WR_RO_PUF_NUM_CHAL     <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_S_INC_ADDR       ])  WR_RO_PUF_S_INC        <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_MAX_CNT_ADDR     ])  WR_RO_PUF_MAX_CNT      <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_SEL1_ADDR        ])  WR_RO_PUF_SEL1         <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_SEL2_ADDR        ])  WR_RO_PUF_SEL2         <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_ECC_ADDR         ])  WR_RO_PUF_ECC          <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_HD_ADDR          ])  WR_RO_PUF_HD           <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_ECC_RATE_ADDR    ])  WR_RO_PUF_ECC_RATE     <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_KEY_ID_ADDR      ])  WR_RO_PUF_KEY_ID       <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_INPUT_KEY0_ADDR  ])  WR_RO_PUF_INPUT_KEY0   <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_INPUT_KEY1_ADDR  ])  WR_RO_PUF_INPUT_KEY1   <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_INPUT_KEY2_ADDR  ])  WR_RO_PUF_INPUT_KEY2   <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_INPUT_KEY3_ADDR  ])  WR_RO_PUF_INPUT_KEY3   <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_CORE_EN_KEY0_ADDR])  WR_RO_PUF_CORE_EN_KEY0 <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_CORE_EN_KEY1_ADDR])  WR_RO_PUF_CORE_EN_KEY1 <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_CORE_EN_KEY2_ADDR])  WR_RO_PUF_CORE_EN_KEY2 <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_CORE_EN_KEY3_ADDR])  WR_RO_PUF_CORE_EN_KEY3 <= `tCQ   IN[31:0];
   if (Wr && decode[WR_RO_PUF_IRQ_EN_ADDR      ])  WR_RO_PUF_IRQ_EN       <= `tCQ   IN[31:0];
   end

// Outputs to RO PUF ============================================================================================================

assign   o_ip_rst     = WR_RO_PUF_CONTROL[0];
         o_ip_en      = WR_RO_PUF_CONTROL[1];
         o_lcs        = WR_RO_PUF_CONTROL[2];
         o_mode       = WR_RO_PUF_CONTROL[4:3];
         o_core_en    = WR_RO_PUF_CONTROL[5];
         o_ecc_en     = WR_RO_PUF_CONTROL[6];
         o_max_cnt_en = WR_RO_PUF_CONTROL[7];
         o_nr         = WR_RO_PUF_CONTROL[8];
         o_lh         = WR_RO_PUF_CONTROL[9];
         o_start      = WR_RO_PUF_CONTROL[10];
         o_load_sel   = WR_RO_PUF_CONTROL[11];
         o_chlg_ctrl  = WR_RO_PUF_CONTROL[13:12];



       .o_chlg_cnt         (chlg_cnt       ),         // wire [xxx:0] if2rp = {  chlg_cnt[MAX_CHL_WIDTH-1:0],
       .o_chlg_ctrl        (chlg_ctrl      ),         //                         chlg_ctrl             [1:0],
       .o_core_en          (core_en        ),         //                         core_en,
       .o_core_en_key      (core_en_key    ),         //                         core_en_key         [127:0],
       .o_ecc_en           (ecc_en         ),         //                         ecc_en,
       .o_ecc_rate         (ecc_rate       ),         //                         ecc_rate              [4:0],
       .o_ecc_wr_data      (ecc_wr_data    ),         //                         ecc_wr_data          [31:0],
       .o_hd_wr_data       (hd_wr_data     )          //                         hd_wr_data           [31:0],
       .o_inc              (inc            ),         //                         inc                   [7:0],
       .o_irq              (ip2intc_irpt   ),         //
       .o_ip_en            (ip_en          ),         //                         ip_en,
       .o_ip_rst           (ip_rst         ),         //                         ip_rst,
       .o_key              (key            ),         //                         key                 [127:0],
       .o_key_id           (key_id         ),         //                         key_id                [4:0],
       .o_lcs              (lcs            ),         //                         lcs,
       .o_lh               (lh             ),         //                         lh,
       .o_load_sel         (load_sel       ),         //                         load_sel,
       .o_max_cnt          (max_cnt        ),         //                         max_cnt [COUNTER_WIDTH-1:0],
       .o_max_cnt_en       (max_cnt_en     ),         //                         max_cnt_en,
       .o_mode             (mode           ),         //                         mode                  [1:0],
       .o_nr               (nr             ),         //                         nr,
       .o_read_reg         (read_reg       ),         //                         read_reg         [8*32-1:0],
       .o_sel01            (sel01          ),         //                         sel01     [2*SEL_WIDTH-1:0],
       .o_sel23            (sel23          ),         //                         sel23     [2*SEL_WIDTH-1:0],
       .o_start            (start          ),         //                         start };


// Inputs from RO PUF ===========================================================================================================

// wire  [266:0] rp2if = { cmp0_data   [15:0],
//                         cmp0_ready,
//                         cmp0_ro_id,
//                         cmp1_data   [15:0],
//                         cmp1_ready,
//                         cmp1_ro_id,
//                         cmp_end,
//                         ecc_rd_data [31:0],
//                         ecc_word_ready,
//                         hd_rd_data  [31:0],
//                         hd_word_ready,
//                         id_rd_data  [31:0],
//                         id_ready,
//                         id_word_ready,
//                         ip_ready,
//                         key_ready,
//                         secure_key  [127:0] };

// Register Reads ===============================================================================================================

always @(posedge clock)
   if (cIO==3'b101)  OUT[31:0] <= `tCQ    {32{(cIO==3'b101) & decode[WR_RO_PUF_CONTROL     ]}} &   WR_RO_PUF_CONTROL
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_NUM_CHAL    ]}} &   WR_RO_PUF_NUM_CHAL
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_S_INC       ]}} &   WR_RO_PUF_S_INC
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_MAX_CNT     ]}} &   WR_RO_PUF_MAX_CNT
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_SEL1        ]}} &   WR_RO_PUF_SEL1
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_SEL2        ]}} &   WR_RO_PUF_SEL2
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_ECC         ]}} &   WR_RO_PUF_ECC
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_HD          ]}} &   WR_RO_PUF_HD
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_ECC_RATE    ]}} &   WR_RO_PUF_ECC_RATE
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_KEY_ID      ]}} &   WR_RO_PUF_KEY_ID
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_INPUT_KEY0  ]}} &   WR_RO_PUF_INPUT_KEY0
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_INPUT_KEY1  ]}} &   WR_RO_PUF_INPUT_KEY1
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_INPUT_KEY2  ]}} &   WR_RO_PUF_INPUT_KEY2
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_INPUT_KEY3  ]}} &   WR_RO_PUF_INPUT_KEY3
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_CORE_EN_KEY0]}} &   WR_RO_PUF_CORE_EN_KEY0
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_CORE_EN_KEY1]}} &   WR_RO_PUF_CORE_EN_KEY1
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_CORE_EN_KEY2]}} &   WR_RO_PUF_CORE_EN_KEY2
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_CORE_EN_KEY3]}} &   WR_RO_PUF_CORE_EN_KEY3
                                        | {32{(cIO==3'b101) & decode[WR_RO_PUF_IRQ_EN      ]}} &   WR_RO_PUF_IRQ_EN

                                        | {32{(cIO==3'b101) & decode[RD_RO_PUF_STATUS      ]}} &   RD_RO_PUF_STATUS
                                        | {32{(cIO==3'b101) & decode[RD_RO_PUF_ECC         ]}} &   RD_RO_PUF_ECC
                                        | {32{(cIO==3'b101) & decode[RD_RO_PUF_ID          ]}} &   RD_RO_PUF_ID
                                        | {32{(cIO==3'b101) & decode[RD_RO_PUF_HD          ]}} &   RD_RO_PUF_HD
                                        | {32{(cIO==3'b101) & decode[RD_RO_PUF_SECURE_KEY0 ]}} &   RD_RO_PUF_SECURE_KEY0
                                        | {32{(cIO==3'b101) & decode[RD_RO_PUF_SECURE_KEY1 ]}} &   RD_RO_PUF_SECURE_KEY1
                                        | {32{(cIO==3'b101) & decode[RD_RO_PUF_SECURE_KEY2 ]}} &   RD_RO_PUF_SECURE_KEY2
                                        | {32{(cIO==3'b101) & decode[RD_RO_PUF_SECURE_KEY3 ]}} &   RD_RO_PUF_SECURE_KEY3;

// Recollate imiso =============================================================================================================

wire  [31:0] IOo  =  Rd ? OUT : 32'h0;

wire         IOk  = (Wr | Rd);

assign      imiso = {IOk, IOo};

endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

`ifdef      verify_ro_puf           // Add this as a Simulator command line define
`timescale  1ns/1ps

module   t;

parameter   PERIOD      =  10;

integer        count, i;

wire   [32:0]  imiso;
reg    [47:0]  imosi;
reg            reset;
reg            clock;

A2_XXXXXX_CoP mut (
   .imiso   (imiso),
   .imosi   (imosi),
   .ok      (puf_flag),
   .reset   (reset),
   .clock   (clock)
   );

initial begin
   clock =  1'b0;
   forever begin
      #(PERIOD/2) clock = 1'b1;
      #(PERIOD/2) clock = 1'b0;
      end
   end

always @(posedge clock) count = reset ? 0 : count +1;

initial begin
   reset =  0;
   #10;
   reset =  1;
   #50;
   reset =  0;
   #10;

   $finish;
   end

endmodule


`endif

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
