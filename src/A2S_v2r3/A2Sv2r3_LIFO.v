/* ******************************************************************************************************************************

   Copyright © 2020 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         :  A2S Stack Machine -- variant 2

   Description          :  LIFO Stack for Data and Return Stacks

*********************************************************************************************************************************

   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

********************************************************************************************************************************

   Variant 2:  6-bit Orders, 32/64-bit only
   Revision3:  Support for spill and fill added

   Prototype:

   A2_LIFO #(32,32) iSS (.LFo(),.LFd(),.LFf(),.LFm(),.iLF(),.cLF(),.eLF(),.clock(),.reset());

********************************************************************************************************************************/
`ifdef synthesis
       `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2_project_settings.vh"
`endif
module A2Sv2r3_LIFO #(
   parameter   WIDTH =  32,
   parameter   DEPTH =  32,            // Max depth is 1024 -- or it won't fit in the Stack Status word!
   parameter   LG2D  = `LG2(DEPTH)
   )(
   output wire [WIDTH-1:0]  LFo,       // Data Out
   output wire [LG2D   :0]  LFd,       // Current Depth
   output wire              LFf,       // Full
   output wire              LFm,       // Empty
   output wire              LFs,       // Stall
   input  wire [WIDTH-1:0] iLF,        // Data In
   input  wire [      1:0] cLF,        // Command
   input  wire             eLF,        // Enable
   input  wire             sLF,        // Spill
   input  wire             fLF,        // Fill
   input  wire             clock,
   input  wire             reset
   );

localparam     [      1:0] HOLD = 2'b00, RPLC  = 2'b01, POP = 2'b10,  PUSH = 2'b11;  // Stack Commands

// Pointer and Flags
reg  [LG2D:0]  PTR,DPSTK,BASE;
reg            EMPTY, FULL, SFR;          // Synchronous flags set on clock edge that pointer advances to FULL or EMPTR State

wire           en     = (cLF!=HOLD) & eLF | SFR,
               we     = (cLF==PUSH) | (cLF==RPLC),
               re     = (cLF==POP );


wire [LG2D:0]  lhs    = en ? PTR : BASE,
               rhs    = (cLF == PUSH) | sLF ?  1
                      : (cLF == POP)  | fLF ? -1
                      :                        0,
               nAddr  = lhs + rhs;

always @(posedge clock)
   if       (reset)  begin
      BASE  <= `tCQ  {LG2D+1{1'b0}};
      PTR   <= `tCQ  {LG2D+1{1'b0}};
      DPSTK <= `tCQ  {LG2D+1{1'b0}};
      EMPTY <= `tCQ  1'b1;
      FULL  <= `tCQ  1'b0;
      SFR   <= `tCQ  1'b0;                // Spill/Fill restore!
      end
   else if  (SFR)                         // Used to restore RFo / LFo after spill/fill operations
      SFR   <= `tCQ  1'b0;
   else if  (sLF) begin                   // Spill
      DPSTK <= `tCQ  DPSTK + 1;
      BASE  <= `tCQ  nAddr;
      SFR   <= `tCQ  1'b1;
      end
   else if  (fLF)  begin                  // Fill
      DPSTK <= `tCQ  DPSTK - 1;
      BASE  <= `tCQ  nAddr;
      SFR   <= `tCQ  1'b1;
      end
   else if  (eLF && (cLF==PUSH) && !FULL ) begin
      EMPTY <= `tCQ  1'b0;
      FULL  <= `tCQ  PTR == {1'b0,{LG2D{1'b1}}};
      DPSTK <= `tCQ  DPSTK + 1;
      PTR   <= `tCQ  nAddr;
      end
   else if  (eLF && (cLF==POP)  && !EMPTY) begin
      FULL  <= `tCQ  1'b0;
      EMPTY <= `tCQ  PTR == {{LG2D{1'b0}},1'b1};
      DPSTK <= `tCQ  DPSTK - 1;
      PTR   <= `tCQ  nAddr;
      end

wire  [WIDTH-1:0] iRF  =  iLF;
wire  [LG2D -1:0] aRF  =  nAddr;
wire              eRF  =  en  | sLF   | fLF | SFR;
wire              rRF  =  re  & ~EMPTY| sLF;
wire              wRF  =  we  & ~FULL | fLF & ~SFR;

reg   [WIDTH-1:0] RF [0:DEPTH-1];
reg   [WIDTH-1:0] RFo;

always @(posedge clock)
   if (eRF)
      if (wRF) begin
         RF[aRF]   <= `tCQ  iRF[WIDTH-1:0];
         RFo       <= `tCQ  iRF[WIDTH-1:0];
         end
      else
         RFo       <= `tCQ   RF[aRF];

// Outputs
assign   LFo    = RFo,
         LFd    = DPSTK[LG2D:0],
         LFf    = FULL,
         LFm    = EMPTY,
         LFs    = SFR;

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
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
