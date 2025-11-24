//==============================================================================================================================
//
//   Copyright © 1996..2024 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name         : Clocked Square Sparrow Prefix Arbiter
//
//   Creation Date        : 5/15/1999
//
//   Added global enable  : 20240126 RTT
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
//   For a four "request" example the module will generate, for each request:
//
//   win[n]  = request[n] & grant[(n+0)%4] & ~request[(n+1)%4] & ~request[(n+2)%4] & ~request[(n+3)%4]
//           | request[n] & grant[(n+1)%4] & ~request[(n+2)%4] & ~request[(n+3)%4]
//           | request[n] & grant[(n+2)%4] & ~request[(n+3)%4]
//           | request[n] & grant[(n+3)%4]
//
//   For Square-Sparrow operation:    The grant signal should be the winner of the previous access
//===============================================================================================================================

`ifdef synthesis //JLPL
    `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh"
    `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_defines.vh"
`else
    `include "A2_project_settings.vh"
    `include "A2_defines.vh"
`endif

module   A2_square_sparrow #(
   parameter NUM_REQUESTS = 16               // Must be a power of two or the wraparound doesn't work.
   )(
   output reg  [NUM_REQUESTS-1:0] win,       // The selected winner of arbitration
   input  wire [NUM_REQUESTS-1:0] req,       // Incoming Requests
   input  wire                    adv,       // Request taken!  Reload Last Register. Tie to '1' if always taken.
   input  wire                    en,        // Global Enable:  if false no win!!
   input  wire                    clock,
   input  wire                    reset
   );

localparam  LG2_REQUESTS  =  `LG2(NUM_REQUESTS);

reg   [NUM_REQUESTS-1:0]   Last;             // Winner from last cycle that had at least one request

integer  i,j;

// This circuit looks like a prefix adder carry chain but with end-around carries at every level
// So every level is "GA" save the last which is just "G".
reg   [NUM_REQUESTS-1:0]   g, gg;      // "generates"
reg   [NUM_REQUESTS-1:0]   a, aa;      // "alives"

always @* begin
   gg =  Last;
   aa = ~req;
   for (i=0; i<LG2_REQUESTS-1; i=i+1) begin
      g = gg;
      a = aa;
      for  (j=0; j<NUM_REQUESTS; j=j+1) begin
         gg[j] = g[j] | a[j] & g[(j + NUM_REQUESTS - (1<<i)) % NUM_REQUESTS];
         aa[j] =        a[j] & a[(j + NUM_REQUESTS - (1<<i)) % NUM_REQUESTS];
         end
      end
   g = gg;
   a = aa;
   for  (j=0; j<NUM_REQUESTS; j=j+1) begin
      gg[j] = g[j] | a[j] & g[(j + NUM_REQUESTS - (1<<(LG2_REQUESTS-1))) % NUM_REQUESTS];
      end
   win   =  {gg[NUM_REQUESTS-2:0],gg[NUM_REQUESTS-1]} & req & {NUM_REQUESTS{en}};
   end

always @(posedge clock)
   if      (reset)         Last  <= `tCQ {{NUM_REQUESTS-1{1'b0}},1'b1};
   else if (|req && adv)   Last  <= `tCQ win;

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
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

