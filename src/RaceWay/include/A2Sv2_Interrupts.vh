/* *******************************************************************************************************************************

   Copyright © 2020 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         :  A2S Stack Machine -- variant 2

   Description          :  Interrupt Controller and Stack Status

*********************************************************************************************************************************

   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

********************************************************************************************************************************/
//
// From reset, start in Supervisor mode.  Turn off to enter user mode.  Interrupts put A2S back into Supervisor mode
// Each Entry point entry is a 4 byte absolute address of the interrupt routine.  If all 32 interrupts are defined then the
// maximum size of the vector table is 128 bytes.  The vector table is protected and is not readable by code operating in
// user mode.  Supervisor code should be potected when a Memory Protection Unit is included in the system.
//
//                            31                                                           1 0
//                            +-------------------------------------------------------+-+-+-+-+
//       Status Register   ST |                  read as zeros                        |W|P|S|I|
//                            +-------------------------------------------------------+-+-+-+-+
//                                                                                      \ \ \ \___ Interrupt Enable
//                                                                                       \ \ \____ User(0), Supervisor(1)
//                                                                                        \ \_____ Prefix Caught
//                                                                                         \______ Wait for Interrupt
//
//                            31            24 23           16 15            8 7             0
//                            +---------------+---------------+---------------+---------------+
// Stack Status Register   SS |    D depth    |    R Depth    |  D Threshold  |  R Threshold  |
//                            +---------------+---------------+---------------+---------------+
//
//                            31 30                                        9 8 7 6 5 4 3 2 1 0
//                            +-+-+---------------------------------------+-+-+-+-+-+-+-+-+-+-+
//     Interrupt Register  IN |X|U|             read as zeros             |F|D|R|D|R|D|R|D|R|S|
//                            +-+-+---------------------------------------+-+-+-+-+-+-+-+-+-+-+
//                              \ \                                         \ \ \ \ \ \ \ \ \ \__ ST: START
//                               \ \                                         \ \ \ \ \ \ \ \ \___ RU: R stack underflow
//                                \ \                                         \ \ \ \ \ \ \ \____ SU: D stack underflow
//                                 \ \                                         \ \ \ \ \ \ \_____ RF: R stack Full
//                                  \ \                                         \ \ \ \ \ \______ SF: D stack Full
//                                   \ \                                         \ \ \ \ \_______ RE: R stack Empty
//                                    \ \                                         \ \ \ \________ SE: D stack Empty
//                                     \ \                                         \ \ \_________ RT: R stack Threshold crossing
//                                      \ \                                         \ \__________ DT: D stack Threshold crossing
//                                       \ \                                         \___________ FX: Floating Point Exception
//                                        \ \____________________________________________________ US: User
//                                         \_____________________________________________________ EX: OR of External Interrupts
//
//                            31 30                                        9 8 7 6 5 4 3 2 1 0
//                            +-+-+---------------------------------------+-+-+-+-+-+-+-+-+-+-+
//     Interrupt Enable    IE |X|U|            read as zeros              |F|D|R|D|R|D|R|D|R|1|
//                            +-+-+---------------------------------------+-+-+-+-+-+-+-+-+-+-+
//
//                            31                                               7 6         1 0
//                            +-------------------------------------------------+---------+-+-+
//       Entry Point       EP |                                                 |  nnnnn  |0|0|
//                            +-------------------------------------------------+---------+-+-+
//                                                                                    \      \____ 4 byte gap
//                                                                                     \__________ Priority encode of interrupts
//
//*******************************************************************************************************************************

localparam  NINT  =   12;
localparam  LINT  =  `LG2(NINT);

// Decodes
wire     dST      =  aIO == aST;
wire     dSS      =  aIO == aST;
wire     dIN      =  aIO == aIN;
wire     dIE      =  aIO == aIE;
wire     dEP      =  aIO == aEP;
wire     swIO     =  eIO & (cIO==IOWRop) &  ST[SVR];    // Supervisor IO write

// Any User attempt to write to a Supervisor location should cause an interrupt
wire     user_intpt  =  eIO   & ~ST[SVR] & aIO[12] & aIO[11];

assign   take_intpt  =  Intpt &  fetch;

// Status Register --------------------------------------------------------------------------------------------------------------
// bit 0    interrupt enable: forced to zero when interrupt taken
// bit 1    SUpervisor mode
// bit 2    Prefix Caught
// bit 3    Wait for Interrupt
always @(posedge clock)
   if (reset)              ST       <= #1  4'b0010;           // Reset is Supervisor mode with Interrupts disabled
   else if (swIO && dST)   ST       <= #1  T[WSTR-1:0];
   else begin
      if (run)             ST[PFC]  <= #1  ((order==PFX) | Prefix & ~rYQ);
      if (intpt)           ST[WFI]  <= #1  1'b0;
      if (take_intpt) begin
                           ST[IEN]  <= #1  1'b0;
                           ST[SVR]  <= #1  1'b0;
                           ST[WFI]  <= #1  1'b0;
                           end
      end

always @* Prefix = ST[PFC];

wire   [WA2S-1:0] STo   =  {{WA2S-WSTR{1'b0}},ST[3:0]};

// Stack Status Register --------------------------------------------------------------------------------------------------------
reg   [15:0]  SS;
always @(posedge clock)
   if (reset)                   SS[15:00]   <= #1 IMM0;
   else if (run && swIO && dSS) SS[15:00]   <= #1 T[15:00];

wire  [WA2S-1:0]  SSo   =  {8'h0|Sdepth, 8'h0|Rdepth, SS[15:00]};
wire  Runderflow_intpt  =   (cR==POP) &  Rmt;
wire  Sunderflow_intpt  =   (cS==POP) &  Smt;
wire  Rthreshold_intpt  =  ((cR==PUSH)|(cR==POP)) & ((8'h0|Rdepth)==SS[15:08]);
wire  Sthreshold_intpt  =  ((cS==PUSH)|(cS==POP)) & ((8'h0|Sdepth)==SS[07:00]);

// Interrupt Enables ------------------------------------------------------------------------------------------------------------
reg   [NINT-1:0] IE;
always @(posedge clock)
   if      (reset)               IE <= #1  IMM0;                     // All INterrupts masked on reset
   else if (run && swIO  && dIE) IE <= #1  {T[31:30],T[NINT-3:0]};

// Interrupt Register -----------------------------------------------------------------------------------------------------------
// External interrupt seen at bit 31
wire  [NINT-1:0]  interrupts  = {intpt,                              // Lowest Priority external interrupt
                                 user_intpt,
`ifdef A2S_ENABLE_FLOATING_POINT
                                 exception_FP,
`else
                                 1'b0,
`endif
                                 Sthreshold_intpt,
                                 Rthreshold_intpt,
                                 Smt,
                                 Rmt,
                                 Sfull,
                                 Rfull,
                                 Sunderflow_intpt,
                                 Runderflow_intpt,                  // Highest Priority
                                 1'b0                                // Reserved for startup
                                 };

reg   [NINT-1:0] IN;
always @(posedge clock)
   if      (reset)               IN[NINT-1:0] <= #1  {NINT{1'b0}};
   else if (run && swIO && dIN)  IN[NINT-1:0] <= #1  IN[NINT-1:0] & ~{T[31:30],T[NINT-3:0]};  // Write '1' per bit to clear
   else if (run)                 IN[NINT-1:0] <= #1  IN[NINT-1:0] |   interrupts;

reg   [LINT-1:0]  icode;
reg               iflag;
always @* (* parallel_case *) case (IN[NINT-1:0] & IE[NINT-1:0])
   12'b???????????1 :  {iflag,icode} = {1'b1,4'd00};  // Highest
   12'b??????????10 :  {iflag,icode} = {1'b1,4'd01};
   12'b?????????100 :  {iflag,icode} = {1'b1,4'd02};
   12'b????????1000 :  {iflag,icode} = {1'b1,4'd03};
   12'b???????10000 :  {iflag,icode} = {1'b1,4'd04};
   12'b??????100000 :  {iflag,icode} = {1'b1,4'd05};
   12'b?????1000000 :  {iflag,icode} = {1'b1,4'd06};
   12'b????10000000 :  {iflag,icode} = {1'b1,4'd07};
   12'b???100000000 :  {iflag,icode} = {1'b1,4'd08};
   12'b??1000000000 :  {iflag,icode} = {1'b1,4'd09};
   12'b?10000000000 :  {iflag,icode} = {1'b1,4'd10};
   12'b100000000000 :  {iflag,icode} = {1'b1,4'd11};  // Lowest
   default          :  {iflag,icode} = {1'b0,4'dx };  // No Interrupt
   endcase

// Interrupt Capture and clear --------------------------------------------------------------------------------------------------
always @(posedge clock)
   if      (reset)                     Intpt <= #1   1'b0;
   else if (run)                       Intpt <= #1  ~take_intpt & (iflag & ST[IEN] | Intpt);

reg   [LINT-1:0]  IV;
always @(posedge clock)
   if      (reset)                     IV <= #1 {LINT{1'b0}};
   else if (run && iflag && ST[IEN])   IV <= #1  icode;

// Interrupt Entry Point --------------------------------------------------------------------------------------------------------
reg   [WA2S-1:0] EP;
always @(posedge clock)
   if      (reset)               EP <= #1  {WA2S-LINT-3{1'b0}};
   else if (run && swIO  && dEP) EP <= #1  T[31:LINT+3];

// Outputs ----------------------------------------------------------------------------------------------------------------------
assign   entry_point          = {EP[31:LINT+2],IV[LINT-1:0],2'b00};

assign   IOo_intpt[WA2S-1:0]  = {WA2S{dST && (cIO==IORDop) &&  ST[SVR]}} &  STo[WA2S-1:0]
                              | {WA2S{dSS && (cIO==IORDop) &&  ST[SVR]}} &  SSo[WA2S-1:0]
                              | {WA2S{dIN && (cIO==IORDop) &&  ST[SVR]}} &  {IN[NINT-1:NINT-2],{WA2S-NINT{1'b0}},IN[NINT-3:0]}
                              | {WA2S{dIE && (cIO==IORDop) &&  ST[SVR]}} &  {IE[NINT-1:NINT-2],{WA2S-NINT{1'b0}},IE[NINT-3:0]}
                              | {WA2S{dEP && (cIO==IORDop) &&  ST[SVR]}} &  {EP[31:LINT+2],{LINT+2{1'b0}}};
                              //;  // Leave the semicolon!   //Joel 8_3_23

assign   IOk_intpt            =  dST | dSS | dIN | dIE | dEP;     // Acknowledge, even if no privilege, so access completes!

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
