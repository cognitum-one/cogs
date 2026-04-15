//===============================================================================================================================
//
//    Copyright � 1996..2010 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//
//    Project Name   : Pipeline 1 to 4 expansion buffer
//
//    Author         : RTT
//    Creation Date  : 5/26/2004
//
//    Description    : Stallable pipestage
//                     This is a pipestage module that when linked in a chain creates a FIFO that may stall in any stage
//                     This can be used as a FIFO or as a stallable pipeline with logic between each stage.
//                     This module reduces an incoming width by a factor of 4 (i.e. 32-bits in -> 8-bits out)
//
//
//===============================================================================================================================
//    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================


module A2_pipe_1to4  #(
   parameter   W  =  8                    // Width of INPUT -- output is 4 times wider
   )(
   input  wire   [W-1:0]   ppl_di,
   input  wire             ppl_wr,        // Like a FIFO push (active)
   output wire             ppl_ir,        // Like a FIFO not full (Input Ready)

   output wire [W*4-1:0]   ppl_do,
   input  wire             ppl_rd,        // Like a FIFO pop  (active)
   output wire             ppl_or,        // Like a FIFO not empty (Output Ready)

   input  wire             ppl_flush,     // Flush the buffer and start anew
   input  wire             ppl_endian,    // Swap the input order in the output word  0:little, 1:big

   input  wire             clock,
   input  wire             reset
   );

localparam  L = 1'b0, H = 1'b1, x = 1'bx;
// Flip-flops ------------------------------------------------------------------------------------------------------------------

reg              [W-1:0]   pipe  [0:3];  // Pipeline register normally one deep
reg              [W-1:0]   skid;         // Slave register for two deep
reg                [2:0]   count;        // Count of available words for output

// Local Nets ------------------------------------------------------------------------------------------------------------------

reg                        ld_skid, ld_pipe;
reg                        inc_by1, dec_by4, dec_by3;
reg                        sel_data;

wire               [3:0]   sel_pipe;

//------------------------------------------------------------------------------------------------------------------------------

always @(posedge clock) begin

   if ( ld_skid )
      skid  <= ppl_di[W-1:0];

   if ( ld_pipe ) begin
      if (sel_pipe[0])  pipe[0][W-1:0]   <=  sel_data ? ppl_di[W-1:0] : skid[W-1:0];   // The data is "mise en place"
      if (sel_pipe[1])  pipe[1][W-1:0]   <=  sel_data ? ppl_di[W-1:0] : skid[W-1:0];   // This is NOT a shift register
      if (sel_pipe[2])  pipe[2][W-1:0]   <=  sel_data ? ppl_di[W-1:0] : skid[W-1:0];   // Allows for partial output when
      if (sel_pipe[3])  pipe[3][W-1:0]   <=  sel_data ? ppl_di[W-1:0] : skid[W-1:0];   // Flush accompanies read
      end
   end

`define TruthTable  {ld_skid,ld_pipe, sel_data, inc_by1,dec_by4,dec_by3}

                                        // ___________ld_skid
                                       // / __________ld_pipe
                                      // / /  ________sel_data
                                     // / /  /  ______inc_by1
always @* begin                     // / /  /  / _____dec_by4
   casez ({count, ppl_wr, ppl_rd}) // / /  /  / / ____dec_by3
                                  // / /  /  / / /
      5'b 0??_0_1 : `TruthTable = { L,L, x, L,L,L };  // Not Full
      5'b 0??_1_? : `TruthTable = { L,H, H, H,L,L };

      5'b 100_0_1 : `TruthTable = { L,L, x, L,H,L };  // Full
      5'b 100_1_0 : `TruthTable = { H,L, x, H,L,L };
      5'b 100_1_1 : `TruthTable = { L,H, H, L,L,H };

      5'b 101_0_1 : `TruthTable = { L,H, L, L,H,L };  // Skidded
      5'b 101_1_0 : `TruthTable = { L,L, x, L,L,L };
      5'b 101_1_1 : `TruthTable = { H,H, L, L,L,H };

      default     : `TruthTable = { L,L, x, L,L,L };
      endcase
   end

always @(posedge clock) begin
   if (reset || ppl_flush)  count <= 3'd 0;
   else begin
      if ( inc_by1 )                      count <= count + 3'd 1;
      if ( dec_by4 )                      count <= count - 3'd 4;
      if ( dec_by3 )                      count <= count - 3'd 3;
      end
   end


assign   sel_pipe = ppl_endian ?
                    {( count[2] |  ~count[1] & ~count[0]),    // sel_pipe[3]: 000 | 100 | 101
                     (~count[2] &  ~count[1] &  count[0]),    // sel_pipe[2]: 001
                     (~count[2] &   count[1] & ~count[0]),    // sel_pipe[1]: 010
                     (~count[2] &   count[1] &  count[0])} :  // sel_pipe[0]: 011
                    {(~count[2] &   count[1] &  count[0]),    // sel_pipe[3]: 011
                     (~count[2] &   count[1] & ~count[0]),    // sel_pipe[2]: 010
                     (~count[2] &  ~count[1] &  count[0]),    // sel_pipe[1]: 001
                     ( count[2] |  ~count[1] & ~count[0])};   // sel_pipe[0]: 000 | 100 | 101

assign   ppl_do[W*4-1:0] = {pipe[3],pipe[2],pipe[1],pipe[0]};
assign   ppl_or          =  (count >  3'd3) | ppl_flush;
assign   ppl_ir          = ~(count == 3'd5);

endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

// Set a command line `define to "verify_A2_1to4buf" to do a standalone module test on this file
`ifdef   verify_a2_1to4buf

A2_1to4buf  #(
   .W          (WIDTH)
   ) u_xx (
   .ppl_di     (ppl_di),
   .ppl_wr     (ppl_wr),        // Like a FIFO push (active)
   .ppl_ir     (ppl_ir)         // Like a FIFO not full (Input Ready)

   .ppl_do     (ppl_do),
   .ppl_rd     (ppl_rd),        // Like a FIFO pop  (active)
   .ppl_or     (ppl_or),        // Like a FIFO not empty (Output Ready)

   .ppl_flush  (ppl_flush),     // Flush the buffer and start anew
   .ppl_endian (ppl_endian),    // Swap the input order in the output word  0:little, 1:big

   .clock      (clock),
   .reset      (reset)
   );


`endif

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

