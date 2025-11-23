//===============================================================================================================================
//
//     Copyright © 2022..2024 Advanced Architectures
//
//     All rights reserved
//     Confidential Information
//     Limited Distribution to authorized Persons Only
//     Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//     Project Name         :  A2S Stack Machine -- variant 2 revision 3
//
//     Description          :  Interrupt Controller and Stack Status
//
//===============================================================================================================================
//
//     THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//     IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//     BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//     TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//     AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//     ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
//===============================================================================================================================
//
// From reset, start in Supervisor mode.  Turn off to enter user mode.  Interrupts put A2S back into Supervisor mode
// Each Entry point entry is a 4 byte absolute address of the interrupt routine.  If all 32 interrupts are defined then the
// maximum size of the vector table is 128 bytes.  The vector table is protected and is not readable by code operating in
// user mode.  Supervisor code should be potected when a Memory Protection Unit is included in the system.
//
//                            31                                                       3 2 1 0
//                            +-------------------------------------------------------+-+-+-+-+
//       Status Register   ST |                  read as zeros                        |W|P|S|I|
//                            +-------------------------------------------------------+-+-+-+-+
//                                                                                      \ \ \ \___ Interrupt Enable
//                                                                                       \ \ \____ User(0), Supervisor(1)
//                                                                                        \ \_____ Prefix Caught
//                                                                                         \______ Wait for Interrupt
//
//                            31 30                  20 19              10 9                0
//                            +-+----------------------+------------------+------------------+
//   D-Stack \ R-stack Status |E|         Depth        |  Fill Threshold  |  Spill Threshold |
//                            +-+----------------------+------------------+------------------+
//                              \_________________________________________________________________E: Spills are Empty
//                                                                                                 when Depth == 0, truly "empty"
//                                                                                              bit30 can be used a "Full"
//
//                            31 30                                     10 9 8 7 6 5 4 3 2 1 0
//                            +-+-+-------------------------------------+-+-+-+-+-+-+-+-+-+-+-+
//     Interrupt Register  IN |E|U|             read as zeros           |S|F|D|R|D|R|D|R|D|R|S|
//                            |X|S|                                     |X|P|T|T|E|E|F|F|U|U|T|
//                            +-+-+-------------------------------------+-+-+-+-+-+-+-+-+-+-+-+
//                              \ \                                       \ \ \ \ \ \ \ \ \ \ \__ ST: START
//                               \ \                                       \ \ \ \ \ \ \ \ \ \___ RU: R stack underflow
//                                \ \                                       \ \ \ \ \ \ \ \ \____ DU: D stack underflow
//                                 \ \                                       \ \ \ \ \ \ \ \_____ RF: R stack Full
//                                  \ \                                       \ \ \ \ \ \ \______ DF: D stack Full
//                                   \ \                                       \ \ \ \ \ \_______ RS: R stack Spill
//                                    \ \                                       \ \ \ \ \________ DS: D stack Spill
//                                     \ \                                       \ \ \ \_________ RF: R stack Fill
//                                      \ \                                       \ \ \__________ DF: D stack Fill
//                                       \ \                                       \ \___________ FX: Floating Point Exception
//                                        \ \                                       \____________ SX: SIMD Exception
//                                         \ \___________________________________________________ US: User
//                                          \____________________________________________________ EX: OR of External Interrupts
//
//                            31 30                                     10 9 8 7 6 5 4 3 2 1 0
//                            +-+-+-------------------------------------+-+-+-+-+-+-+-+-+-+-+-+
//     Interrupt Enable    IE |X|U|            read as zeros            |S|F|D|R|D|R|D|R|D|R|1|
//                            +-+-+-------------------------------------+-+-+-+-+-+-+-+-+-+-+-+
//
//                            31                                               7 6         1 0
//                            +-------------------------------------------------+---------+-+-+
//       Entry Point       EP |                                                 |  nnnnn  |0|0|
//                            +-------------------------------------------------+---------+-+-+
//                                                                                    \      \____ 4 byte gap
//                                                                                     \__________ Priority encode of interrupts
//
//   Spill and Fill Pointers   31                                                        2 1 0
//                            +-----------------------------------------------------------+-+-+
//               SPD   SPR    |                                                           |0|0|
//               FPD   FPR    +-----------------------------------------------------------+-+-+
//
//*******************************************************************************************************************************

localparam  NUM_INTPTS  =  13;
localparam  LG2_INTPTS  = `LG2(NUM_INTPTS);
localparam  ADR_BASE    = 13'h0000; //JLPL
localparam  ADR_RANGE   = 10; //JLPL
localparam  LG2_RANGE   = `LG2(ADR_RANGE);
// Locals -----------------------------------------------------------------------------------------------------------------------

// Nets
// Stack Status
reg   [      WA2S-1:0]  RS, DS, RP, RC, DP, DC, EP;
reg   [NUM_INTPTS-1:0]  IN, IE, IV;

wire  Runder_intpt, Dunder_intpt, Rspill_intpt, Dspill_intpt, Rfill_intpt, Dfill_intpt, Rfull_intpt, Dfull_intpt;

// Interface decoding -----------------------------------------------------------------------------------------------------------
wire                  in_range   =  aIO[12:LG2_RANGE] == ADR_BASE[12:LG2_RANGE];
wire  [ADR_RANGE-1:0]      dec   =  in_range << aIO[LG2_RANGE-1:0];
wire                        wr   =  eIO & (cIO[1:0]==2'b10);
wire                        rd   =  eIO & (cIO[1:0]==2'b01);
assign               IOk_intpt   =  (wr | rd) & |dec[ADR_RANGE-1:0];
wire                      swIO   =  wr  & `ST_SVR;                           // Supervisor IO write

// ------------------------------------------------------------------------------------------------------------------------------
// Any User attempt to write to a Supervisor location should cause an interrupt
wire  user_intpt  =  eIO   & ~`ST_SVR & aIO[12] & aIO[11];

wire  take_intpt  =  Intpt &  fetch;

// Status Register --------------------------------------------------------------------------------------------------------------
// bit 0    interrupt enable: forced to zero when interrupt taken
// bit 1    SUpervisor mode
// bit 2    Prefix Caught
// bit 3    Wait for Interrupt

always @(posedge clock)
   if       (reset)           ST       <= `tCQ  4'b0010;           // Reset is Supervisor mode with Interrupts disabled
   else if  (wr && dec[`iST]) ST       <= `tCQ  T0[3:0];
   else begin
      if (ckePfx)            `ST_PFC   <= `tCQ  prefix_order | `ST_PFC & ~rYQ;
      if (intpt)             `ST_WFI   <= `tCQ  1'b0;
      if (take_intpt) begin  `ST_IEN   <= `tCQ  1'b0;   // On Interrupt:  Disable further Interrupts
                             `ST_SVR   <= `tCQ  1'b1;   //                Force Supervisor mode
                             `ST_WFI   <= `tCQ  1'b0;   //                Clear Wait for Interrupt
                              end
      end

always @* Prefix = `ST_PFC;

// R-Stack Status Register ------------------------------------------------------------------------------------------------------

always @(posedge clock)
   if      (reset)                  RS <= `tCQ {12'h000,10'b1000000000,10'b1000000000}; // Set empty & Thresholds halfway
   else if (run && wr && dec[`iRS]) RS <= `tCQ T0;

assign
   Runder_intpt   =  (cR==POP)  &  `RS_SMT,
   Rspill_intpt   =  (cR==PUSH) & ((10'h0 | Rdepth) == `RS_STH),
    Rfull_intpt   =   `RS_STD == DRSK,                               // DRSK = Depth Return Stack
    Rfill_intpt   =  (cR==POP)  & ((10'h0 | Rdepth) == `RS_FTH);

// D-Stack Status Register ------------------------------------------------------------------------------------------------------

always @(posedge clock)
   if      (reset)                  DS <= `tCQ {12'h000,10'b1000000000,10'b1000000000}; // Set empty & Thresholds halfway
   else if (run && wr && dec[`iDS]) DS <= `tCQ T0;

assign
   Dunder_intpt   =  (cD==POP)  &  `DS_SMT,
   Dspill_intpt   =  (cD==PUSH) & ((10'h0 | Ddepth) == `DS_STH),
    Dfull_intpt   =   `DS_STD == DDSK,                               // DRSK = Depth Data Stack
    Dfill_intpt   =  (cD==POP)  & ((10'h0 | Ddepth) == `DS_FTH);

// Interrupt Register -----------------------------------------------------------------------------------------------------------
// External interrupt seen at bit 31
wire  [NUM_INTPTS-1:0]  interrupts  = {intpt,            // 31 -- 0030        // Lowest Priority external interrupt
                                       user_intpt,       // 30 -- 002c
`ifdef   A2S_ENABLE_SIMD
                                       SIMDex,           // 10 -- 0028
`else
                                       1'b0,
`endif //A2S_ENABLE_SIMD
`ifdef   A2S_ENABLE_FLOATING_POINT
                                       FPex,             // 9  -- 0024
`else
                                       1'b0,
`endif //A2S_ENABLE_FLOATING_POINT
                                       Dfill_intpt,      // 8  -- 0020
                                       Rfill_intpt,      // 7  -- 001c
                                       Dspill_intpt,     // 6  -- 0018
                                       Rspill_intpt,     // 5  -- 0014
                                       Dfull_intpt,      // 4  -- 0010
                                       Rfull_intpt,      // 3  -- 000c
                                       Dunder_intpt,     // 2  -- 0008
                                       Runder_intpt,     // 1  -- 0004        // Highest Priority
                                       1'b0              // 0  -- 0000        // Reserved for startup
                                       };

wire  clearIN  =  run & wr & dec[`iIN];
wire  [NUM_INTPTS-1:0] nIN = IN[NUM_INTPTS-1:0] & ~{T0[31:30],T0[NUM_INTPTS-1:0]};
always @(posedge clock)
   if      (reset)   IN[NUM_INTPTS-1:0] <= `tCQ    {NUM_INTPTS{1'b0}};
   else if (clearIN) IN[NUM_INTPTS-1:0] <= `tCQ  IN[NUM_INTPTS-1:0] & ~{T0[31:30],T0[NUM_INTPTS-3:0]};  // Write '1' per bit to clear
   else if (run)     IN[NUM_INTPTS-1:0] <= `tCQ  IN[NUM_INTPTS-1:0] |   interrupts;

// Interrupt Enables ------------------------------------------------------------------------------------------------------------

always @(posedge clock)
   if      (reset)                  IE  <= `tCQ IMM0;
   else if (run && wr && dec[`iIE]) IE  <= `tCQ {T0[31:30],T0[NUM_INTPTS-3:0]};

wire  [NUM_INTPTS-1:0]  enabled_intpts =  IN[NUM_INTPTS-1:0] & IE[NUM_INTPTS-1:0];
reg   [LG2_INTPTS-1:0]  icode;
reg                     iflag;

always @* begin
   iflag = 1'b0;
   icode = {LG2_INTPTS{1'b1}};
   for (i=0;i<NUM_INTPTS;i=i+1)
      if (!iflag) begin
         iflag = enabled_intpts[i];
         icode = i;
         end
   end

// Interrupt Capture and clear --------------------------------------------------------------------------------------------------
always @(posedge clock)
   if      (reset)                     Intpt <= `tCQ   1'b0;
   else if (run)                       Intpt <= `tCQ  ~take_intpt & (iflag & `ST_IEN | Intpt);

always @(posedge clock)
   if      (reset)                     IV    <= `tCQ {LG2_INTPTS{1'b0}};
   else if (run && iflag && `ST_IEN)   IV    <= `tCQ  icode;

// Interrupt Entry Point --------------------------------------------------------------------------------------------------------
always @(posedge clock)
   if      (reset)                     EP    <= `tCQ  {WA2S-LG2_INTPTS-3{1'b0}};
   else if (run && swIO  && dec[`iEP]) EP    <= `tCQ  T0[31:LG2_INTPTS+3];

`ifdef   A2S_ENABLE_SPILL_FILL
// Spill and Fill Pointers ------------------------------------------------------------------------------------------------------
// 'inc' and 'dec' signals come from Extensions

always @(posedge clock)
   if      (run && wr  && dec[`iRP])   RP  <= `tCQ  T0[31:0];     // Spill/Fill Pointer  R-Stack
   else if (run && incRP)              RP  <= `tCQ  RP + 4;       // 4 byte increment per word
   else if (run && decRP)              RP  <= `tCQ  RP - 4;       // 4 byte decrement per word

always @(posedge clock)
   if      (run && wr  && dec[`iRC])   RC  <= `tCQ  T0[31:0];     // Spill/Fill Dipstick R-Stack
   else if (run && incRC)              RC  <= `tCQ  RC + 4;       // 4 word increment per spill instruction
   else if (run && decRC)              RC  <= `tCQ  RC - 4;       // 4 word decrement per  fill instruction

always @(posedge clock)
   if      (run && wr  && dec[`iDP])   DP  <= `tCQ  T0[31:0];     // Spill/Fill Pointer  D-Stack
   else if (run && incDP)              DP  <= `tCQ  DP + 4;       // 4 byte increment per word
   else if (run && decDP)              DP  <= `tCQ  DP - 4;       // 4 byte decrement per word

always @(posedge clock)
   if      (run && wr  && dec[`iDC])   DC  <= `tCQ  T0[31:0];     // Spill/Fill Dipstick D-Stack
   else if (run && incDC)              DC  <= `tCQ  DC + 4;       // 4 word increment per spill instruction
   else if (run && decDC)              DC  <= `tCQ  DC - 4;       // 4 word decrement per  fill instruction

wire  [9:0] sIO      = (in_range & rd & `ST_SVR) << aIO[3:0];
A2_mux #(10,32) iIOI (  .z(IOo_intpt[WA2S-1:0]),
                        .s(sIO[9:0]),
                        .d({ DC[WA2S-1:0], DP[WA2S-1:0], RC[WA2S-1:0], RP[WA2S-1:0],
`else

wire  [9:0] sIO      = (in_range & rd & `ST_SVR) << aIO[3:0];
A2_mux #( 6,32) iIOI (  .z(IOo_intpt[WA2S-1:0]),
                        .s(sIO[5:0]),
                        .d({
`endif //A2S_ENABLE_SPILL_FILL
                             EP[31:LG2_INTPTS+2],{LG2_INTPTS+2{1'b0}},
                             IE[NUM_INTPTS-1:NUM_INTPTS-2],{WA2S-NUM_INTPTS{1'b0}},IE[NUM_INTPTS-3:0],
                             IN[NUM_INTPTS-1:NUM_INTPTS-2],{WA2S-NUM_INTPTS{1'b0}},IN[NUM_INTPTS-3:0],
                             DS[WA2S-1:0],
                             RS[WA2S-1:0],
                             28'h0,ST[3:0]
                           }));

assign   entry_point = {`EP_EPB,IV[LG2_INTPTS-1:0],2'b00};

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
