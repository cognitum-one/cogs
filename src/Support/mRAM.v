//==============================================================================================================================
//
//   Copyright © 1996..2022 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name         : masked RAM  :  Write mask controls bit-level control of writes
//
//===============================================================================================================================
//      THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//
//===============================================================================================================================

`ifdef  synthesis //JLPL
//   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh"
`else
//   `include "A2_project_settings.vh"
`endif

module   mRAM #(
   parameter   DEPTH = 512,
   parameter   WIDTH = 128,
   parameter   LG2D  = `LG2(DEPTH),
   parameter   WORDS = WIDTH / 32
   )(
   output reg  [WIDTH-1:0] RAMo,    // Data Output /* MULIT-CYCLE NET */
   input  wire [WIDTH-1:0] iRAM,    // Data Input
   input  wire [WIDTH-1:0] mRAM,    // Write Mask per word
   input  wire [LG2D-1 :0] aRAM,    // Address
   input  wire             eRAM,    // Enable
   input  wire             wRAM,    // Write
   input  wire             clock
   );

integer  i;

wire     RAM_clock;

preicg iCG0 (.ECK(RAM_clock), .CK(clock), .E(eRAM), .SE(1'b0));

reg [WIDTH-1:0] RAM [0:DEPTH-1];

always @(posedge RAM_clock)
   if (eRAM) begin
//      RAMo <=  `tCQ RAM[aRAM];
      if (wRAM) begin
         for (i=0; i<WIDTH; i=i+1)
            if (mRAM[i])  begin
               RAM[aRAM][i] <= `tCQ  iRAM[i];
               RAMo[i]      <= `tCQ  iRAM[i];
               end
            else
               RAMo[i]      <= `tCQ  RAM[aRAM][i];
         end
      else
         RAMo <=  `tCQ RAM[aRAM];
      end


endmodule


`ifndef   PREICG
`define   PREICG
module preicg (
   output wire ECK,
   input  wire E, SE,
   input  wire CK
   );
reg    q;

always @* if (!CK) q = (E | SE);

assign ECK = CK & q;

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
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
