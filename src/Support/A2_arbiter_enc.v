/* ==============================================================================================================================

   Copyright © 2000..2024 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : Clocked Square Sparrow Prefix Arbiter with encoded output and a valid bit

   Creation Date        : 5/15/2000

=================================================================================================================================
      THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
=================================================================================================================================

   For a four "request" example the module will generate, for each request:

   win =  encode  ( request[n] & grant[(n+0)%4] & ~request[(n+1)%4] & ~request[(n+2)%4] & ~request[(n+3)%4]
                  | request[n] & grant[(n+1)%4] & ~request[(n+2)%4] & ~request[(n+3)%4]
                  | request[n] & grant[(n+2)%4] & ~request[(n+3)%4]
                  | request[n] & grant[(n+3)%4] )

   For Square-Sparrow operation:    The grant signal is the winner of the previous access
===============================================================================================================================*/

`ifdef  synthesis //JLPL
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2_project_settings.vh"
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2_defines.vh"
`else
   `include "A2_project_settings.vh"
   `include "A2_defines.vh"
`endif

//`include "A2_defines.vh"
//`ifndef  tCQ
//`define  tCQ   #1
//`endif

module   A2_arbiter_enc #(
   parameter NUM_REQUESTS =  16,
   parameter LG2_REQUESTS = `LG2(NUM_REQUESTS)
   )(
   output wire [LG2_REQUESTS  :0] win,       // Encoded winner of arbitration : msb is 'found'
   input  wire [NUM_REQUESTS-1:0] req,       // Incoming Requests
   input  wire                    adv,       // Request taken!  Reload Last Register. Tie to '1' if always taken.
   input  wire                    en,        // Global Enable:  if false no win!!
   input  wire                    clock,
   input  wire                    reset
   );

reg   [NUM_REQUESTS-1:0]   Last;             // Winner from last cycle that had at least one request
reg                        found;

integer  i,j,k;

always @* begin
   // Scan requests
   k        =  0;
   found    =  1'b0;
   for (i=0; (i<NUM_REQUESTS) && !found; i=i+1) begin
      `tCQ;
      j = (Last + i + 1) % NUM_REQUESTS;
      if (en && !found && req[j]) begin
         k        =  j;
         found    =  1'b1;
         end
      end
   end

always @(posedge clock)
   if      (reset)         Last  <= `tCQ {LG2_REQUESTS{1'b1}};
   else if (found && adv)  Last  <= `tCQ  k[LG2_REQUESTS-1:0];

assign   win = {found,k[LG2_REQUESTS-1:0]};

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
`ifdef STUFF
   abcdefgh    ijk x
   00000001    000 1    i = a | b | c | d
   00000010    001 1    j = a | b | e | f
   00000100    010 1    k = a | c | e | g
   00001000    011 1
   00010000    100 1    x = i | j | k | h
   00100000    101 1
   01000000    110 1
   10000000    111 1

`endif
