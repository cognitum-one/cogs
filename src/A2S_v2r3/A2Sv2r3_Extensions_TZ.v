/* ******************************************************************************************************************************

   Copyright © 2022 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         :  A2S Stack Machine -- variant 2 revision 3

   ==========================================================================
                SPECIAL VERSION FOR PROJECT NEWPORT TILE-ZERO
   ==========================================================================
   Description          :  Shifts, ByteSwap, Condition Codes, Population Count,
                           Single precision Floating Point, Integer Multiply and Integer Divide Arithmetic Units

*********************************************************************************************************************************

   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

*********************************************************************************************************************************
      NOTES:

   Byte Extract:  addr dup @ bx  where: bx : (n1 n2 -- b1 = n2[8*n1+:8])   swap 3 & 3 << >>           unsigned char
                  addr dup @ bsx                                           swap 3 & ~ 3 << << 3 >>      signed byte

                  lit dup get swap lit MT [addr, 3] & ext ext rtn nop MT ["lsli_3", "lsrt"]     4 x 32-bits

********************************************************************************************************************************/
genvar g;

// Collect shift data
//wire  [WA2S-1:0]  sdata    = !eFN_order  ? {WA2S{1'b0}} : (fn[15:12] == 4'hf ? T1[WA2S-1:0] : T0[WA2S-1:0] );
//wire  [LA2S-1:0]  distance = !eFN_order  ? {LA2S{1'b0}} : (fn[15:12] == 4'hf ? T0[LA2S-1:0] : fn[LA2S-1:0] );

reg   sh_imm, sh_rel, rotating, shift_left, arithmetic;

always @* if (eFN_order) casez (fn[15:0])
   RORI  :  {sh_imm, sh_rel, rotating, shift_left, arithmetic}  = {1'b1, 1'b0, 1'b1, 1'b0, 1'b0};
   ROLI  :  {sh_imm, sh_rel, rotating, shift_left, arithmetic}  = {1'b1, 1'b0, 1'b1, 1'b1, 1'b0};
   LSRI  :  {sh_imm, sh_rel, rotating, shift_left, arithmetic}  = {1'b1, 1'b0, 1'b0, 1'b0, 1'b0};
   LSLI  :  {sh_imm, sh_rel, rotating, shift_left, arithmetic}  = {1'b1, 1'b0, 1'b0, 1'b1, 1'b0};
   ASRI  :  {sh_imm, sh_rel, rotating, shift_left, arithmetic}  = {1'b1, 1'b0, 1'b0, 1'b0, 1'b1};
   ROR   :  {sh_imm, sh_rel, rotating, shift_left, arithmetic}  = {1'b0, 1'b1, 1'b1, 1'b0, 1'b0};
   ROL   :  {sh_imm, sh_rel, rotating, shift_left, arithmetic}  = {1'b0, 1'b1, 1'b1, 1'b1, 1'b0};
   LSR   :  {sh_imm, sh_rel, rotating, shift_left, arithmetic}  = {1'b0, 1'b1, 1'b0, 1'b0, 1'b0};
   LSL   :  {sh_imm, sh_rel, rotating, shift_left, arithmetic}  = {1'b0, 1'b1, 1'b0, 1'b1, 1'b0};
   ASR   :  {sh_imm, sh_rel, rotating, shift_left, arithmetic}  = {1'b0, 1'b1, 1'b0, 1'b0, 1'b1};
   default  {sh_imm, sh_rel, rotating, shift_left, arithmetic}  = {1'b0, 1'b0, 1'b0, 1'b0, 1'b0};
   endcase
   else     {sh_imm, sh_rel, rotating, shift_left, arithmetic}  = {1'b0, 1'b0, 1'b0, 1'b0, 1'b0};

wire  [WA2S-1:0]  sdata    =  sh_imm   ?  T0[WA2S-1:0]
                           :  sh_rel   ?  T1[WA2S-1:0]      : {WA2S{1'b0}};

wire  [LA2S-1:0]  distance =  sh_imm   ?  fn[LA2S-1:0]
                           :  sh_rel   ?  T0[LA2S-1:0]      : {LA2S{1'b0}};

wire  [LA2S  :0]  samount  = ({LA2S{shift_left}} ^ distance ) + (shift_left ? 1'b1 : 1'b0);     // 2's comp if shifting left
wire                 fill  =  arithmetic & sdata[WA2S-1];

wire  [WA2S-1:0]  level0   =  samount[0] ?  {    sdata[00], sdata[31:01]}  :   sdata[31:00];
wire  [WA2S-1:0]  level1   =  samount[1] ?  {level0[01:00],level0[31:02]}  :  level0[31:00];
wire  [WA2S-1:0]  level2   =  samount[2] ?  {level1[03:00],level1[31:04]}  :  level1[31:00];
wire  [WA2S-1:0]  level3   =  samount[3] ?  {level2[07:00],level2[31:08]}  :  level2[31:00];
wire  [WA2S-1:0]  rotated  =  samount[4] ?  {level3[15:00],level3[31:16]}  :  level3[31:00];

wire  [WA2S-1:0]  mask     =  rotating       ?  {WA2S{1'b1}}
                           :                    {WA2S{shift_left}} ^ ({WA2S{1'b 1}} >> samount);   // invert for shift left!
wire  [WA2S-1:0]  shift    =  mask  &  rotated  |  ~mask  & {WA2S{fill}};
wire  [WA2S-1:0]  swapb    =  {T0[7:0],T0[15:8],T0[23:16],T0[31:24]};
wire  [WA2S-1:0]  rev      =  {T0[00],T0[01],T0[02],T0[03],T0[04],T0[05],T0[06],T0[07],
                               T0[08],T0[09],T0[10],T0[11],T0[12],T0[13],T0[14],T0[15],
                               T0[16],T0[17],T0[18],T0[19],T0[20],T0[21],T0[22],T0[23],
                               T0[24],T0[25],T0[26],T0[27],T0[28],T0[29],T0[30],T0[31]};
wire   [2*WA2S-1:0] shuf;
for (g=0; g<(2*WA2S); g=g+2) begin : g_shuf
   assign shuf[g+1:g]  =  {T1[g/2], T0[g/2]};
   end

wire  [WA2S-1:0]  cctest      = ({{256-NCCI{1'b0}},`ST_WFI,ccode[NCCI-1:1],(|ccode[NCCI:1])} >> fn[7:0]) ? TRUE : FALSE;


integer i;

reg   [2:0] xs;
reg   [6:0] sum;

wire  decode_pops =  eFN_order && (fn[15:2] == POPC[15:2]);

always @*
   if (!decode_pops)
      sum = 7'b000_0000;
   else (* parallel_case *) case (fn[1:0])
      2'b00 :  begin
               sum  = 7'b000_0000;
               for(i=0;i<32;i=i+1)
                  sum = sum + T0[i];
               end
      2'b01 :  begin
               sum   = 7'b100_0000;         // load -64, to compensate for lack of sign extension
               for(i=0;i<16;i=i+1) begin
                  (* parallel_case *) case ({T0[2*i+1],T0[2*i],T1[2*i+1],T1[2*i]})    // Excess Sum
                  //  mask_data
                     4'b00_00                               : xs = 3'b010; // -2 = 2 excess 0s
                     4'b10_00, 4'b10_10, 4'b01_00, 4'b01_01 : xs = 3'b011; // -1 = 1 excess 0
                     default                                  xs = 3'b100; //  0 = equal number of 0s and 1s
                     4'b10_01, 4'b10_11, 4'b01_10, 4'b01_11 : xs = 3'b101; //  1 = 1 excess 1
                     4'b00_11                               : xs = 3'b110; //  2 = 2 excess 1s
                     endcase
                  sum = sum + xs[2:0];
                  end
               end
      2'b10 :  (* parallel_case *) casez (T0)
               32'b00000000000000000000000000000000  : sum = 7'd32;
               32'b00000000000000000000000000000001  : sum = 7'd31;
               32'b0000000000000000000000000000001?  : sum = 7'd30;
               32'b000000000000000000000000000001??  : sum = 7'd29;
               32'b00000000000000000000000000001???  : sum = 7'd28;
               32'b0000000000000000000000000001????  : sum = 7'd27;
               32'b000000000000000000000000001?????  : sum = 7'd26;
               32'b00000000000000000000000001??????  : sum = 7'd25;
               32'b0000000000000000000000001???????  : sum = 7'd24;
               32'b000000000000000000000001????????  : sum = 7'd23;
               32'b00000000000000000000001?????????  : sum = 7'd22;
               32'b0000000000000000000001??????????  : sum = 7'd21;
               32'b000000000000000000001???????????  : sum = 7'd20;
               32'b00000000000000000001????????????  : sum = 7'd19;
               32'b0000000000000000001?????????????  : sum = 7'd18;
               32'b000000000000000001??????????????  : sum = 7'd17;
               32'b00000000000000001???????????????  : sum = 7'd16;
               32'b0000000000000001????????????????  : sum = 7'd15;
               32'b000000000000001?????????????????  : sum = 7'd14;
               32'b00000000000001??????????????????  : sum = 7'd13;
               32'b0000000000001???????????????????  : sum = 7'd12;
               32'b000000000001????????????????????  : sum = 7'd11;
               32'b00000000001?????????????????????  : sum = 7'd10;
               32'b0000000001??????????????????????  : sum = 7'd09;
               32'b000000001???????????????????????  : sum = 7'd08;
               32'b00000001????????????????????????  : sum = 7'd07;
               32'b0000001?????????????????????????  : sum = 7'd06;
               32'b000001??????????????????????????  : sum = 7'd05;
               32'b00001???????????????????????????  : sum = 7'd04;
               32'b0001????????????????????????????  : sum = 7'd03;
               32'b001?????????????????????????????  : sum = 7'd02;
               32'b01??????????????????????????????  : sum = 7'd01;
               default                                 sum = 7'd00;
               endcase
      2'b11 :  (* parallel_case *) casez (T0)
               32'b00000000000000000000000000000000  : sum = 7'd32;
               32'b10000000000000000000000000000000  : sum = 7'd31;
               32'b?1000000000000000000000000000000  : sum = 7'd30;
               32'b??100000000000000000000000000000  : sum = 7'd29;
               32'b???10000000000000000000000000000  : sum = 7'd28;
               32'b????1000000000000000000000000000  : sum = 7'd27;
               32'b?????100000000000000000000000000  : sum = 7'd26;
               32'b??????10000000000000000000000000  : sum = 7'd25;
               32'b???????1000000000000000000000000  : sum = 7'd24;
               32'b????????100000000000000000000000  : sum = 7'd23;
               32'b?????????10000000000000000000000  : sum = 7'd22;
               32'b??????????1000000000000000000000  : sum = 7'd21;
               32'b???????????100000000000000000000  : sum = 7'd20;
               32'b????????????10000000000000000000  : sum = 7'd19;
               32'b?????????????1000000000000000000  : sum = 7'd18;
               32'b??????????????100000000000000000  : sum = 7'd17;
               32'b???????????????10000000000000000  : sum = 7'd16;
               32'b????????????????1000000000000000  : sum = 7'd15;
               32'b?????????????????100000000000000  : sum = 7'd14;
               32'b??????????????????10000000000000  : sum = 7'd13;
               32'b???????????????????1000000000000  : sum = 7'd12;
               32'b????????????????????100000000000  : sum = 7'd11;
               32'b?????????????????????10000000000  : sum = 7'd10;
               32'b??????????????????????1000000000  : sum = 7'd09;
               32'b???????????????????????100000000  : sum = 7'd08;
               32'b????????????????????????10000000  : sum = 7'd07;
               32'b?????????????????????????1000000  : sum = 7'd06;
               32'b??????????????????????????100000  : sum = 7'd05;
               32'b???????????????????????????10000  : sum = 7'd04;
               32'b????????????????????????????1000  : sum = 7'd03;
               32'b?????????????????????????????100  : sum = 7'd02;
               32'b??????????????????????????????10  : sum = 7'd01;
               default                                 sum = 7'd00;
               endcase
      endcase

wire  [WA2S-1:0]  popsum   =  {{WA2S-7{sum[6]}},sum[6:0]};

// SPILL_FILL ===================================================================================================================

//JLPL - 11_3_25 - Comment out multiple drivers below
//assign   spilD    = (order == EXT) & (fn == SPILD),
//         fillD    = (order == EXT) & (fn == FILLD),
//         spilR    = (order == EXT) & (fn == SPILR),
//         fillR    = (order == EXT) & (fn == FILLD),
//         SFstall  = spilD | spilR;

// Integer Multiply and Divide ==================================================================================================

wire  [2*WA2S-1:0]   MDo;
wire                 MDor, MDir;

wire                 eIMD     =  eFN_order & (fn[15:5]==MPY[15:5]);  // This covers 'MPY' through 'QUO'
assign               IMDstall =  eIMD & ~MDor;

A2_muldiv_r2 i_md (
   .result (MDo  ),
   .ordy   (MDor ),
   .read   (1'b1 ),
   .irdy   (MDir ),
   .load   (eIMD ),
   .left   (T1   ),
   .right  (T0   ),
   .fn     ({~fn[4],fn[1:0]}),   // mul/div,  signs
   .reset  (reset),
   .clock  (clock)
   );

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
