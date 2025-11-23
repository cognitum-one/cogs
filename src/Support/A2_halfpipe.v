//===============================================================================================================================
//
//    Copyright © 1996..2010 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//
//    Project Name   : Pipeline
//
//    Author         : RTT
//    Creation Date  : 5/26/2004
//
//    Description    : Stallable pipestage
//                     This is a pipestage module that when linked in a chain creates a FIFO that may stall_i in any stage
//                     This can be used as a FIFO or as a stallable pipeline with logic between each stage.
//
//                     THIS VERSION ASSUMES THAT THE NORMAL "pipe" REGISTER IS EXTERNAL.  THIS ALLOWS A SYNCHRONOUS RAM, FOR
//                     EXAMPLE, TO BE PLACED IN THE PIPELINE
//
//===============================================================================================================================
//    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//==============================================================================================================================

`ifdef  synthesis //JLPL
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2_project_settings.vh"
`else
   `include "A2_project_settings.vh"
`endif

//`ifndef  tCQ
//`define  tCQ   #1
//`endif

module A2_halfpipe #(
   parameter   WIDTH       = 8
   )(
   input   [WIDTH-1:0]  hdi,        // Halfpipe data in
   input                hwr,        // Halfpipe write
   output               hir,        // Halfpipe input ready

   output  [WIDTH-1:0]  hdo,        // Halfpipe data out : mux between Skid and hdi
   output               hor,        // Halfpipe output ready
   output               hld,        // Halfpipe Load the external "pipe" register
   input                hrd,        // Halfpipe read

   input                clock,
   input                reset
   );

localparam  MPT   =  0,             // Pipestage is empty
            ONE   =  1,             // Pipestage has one word  (in 'pipe')
            TWO   =  2;             // Pipestage has two words (one in 'skid' one in 'pipe')

reg     [WIDTH-1:0]  Skid;          // Slave register for two deep
reg           [2:0]  FSM;           // State Machine

always @(posedge clock)
   if (  hwr && !hrd && FSM[ONE]) Skid[WIDTH-1:0] <=  `tCQ hdi[WIDTH-1:0];

assign   hdo[WIDTH-1:0]    =  FSM[TWO]  ?  Skid[WIDTH-1:0] : hdi[WIDTH-1:0];

always @(posedge clock) begin
   if (reset) begin
      FSM   <= `tCQ 3'd 1 << MPT;
      end
   else case (1'b 1)
      FSM[MPT]:  if ( hwr         ) FSM[2:0] <= `tCQ 3'd 1 << ONE;
      FSM[ONE]:  if ( hwr && !hrd ) FSM[2:0] <= `tCQ 3'd 1 << TWO;
            else if (!hwr &&  hrd ) FSM[2:0] <= `tCQ 3'd 1 << MPT;
      FSM[TWO]:  if (         hrd ) FSM[2:0] <= `tCQ 3'd 1 << ONE;
      default                       FSM[2:0] <= `tCQ 3'b x;
      endcase
   end

assign   hir   = ~FSM[TWO];
assign   hor   = ~FSM[MPT];
assign   hld   =  hwr &         FSM[MPT]
               |  hwr &  hrd &  FSM[ONE]
               |         hrd &  FSM[TWO];

endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

