//===============================================================================================================================
//
//    Copyright © 1996..2013 Advanced Architectures
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
//    Creation Date  : 5/26/2006
//
//    Description    : Stallable pipestage
//                     This is a pipestage module that when linked in a chain creates a FIFO that may ppl_stall in any stage
//                     This can be used as a FIFO or as a ppl_stallable pipeline with logic between each stage.
//
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
   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh"
`else
   `include "A2_project_settings.vh"
`endif

//`ifndef  tCQ
//`define  tCQ   #1
//`endif

module A2_pipe_mux21 #(
   parameter   WIDTH       = 8,
   parameter   ARB_TYPE    = "AoverB"     // "AoverB" or "ALTERNATING"
   )(
   output  [WIDTH-1:0]  pdo,
   output               por,              // Like a FIFO not empty (Output Ready)
   input                prd,              // Like a FIFO pop

   input   [WIDTH-1:0]  pdiA,
   input                pwrA,             // Like a FIFO push
   output               pirA,             // Like a FIFO not full  (Input Ready)

   input   [WIDTH-1:0]  pdiB,
   input                pwrB,             // Like a FIFO push
   output               pirB,             // Like a FIFO not full  (Input Ready)

   output               psel,
   input                clock,
   input                reset
   );

wire  [WIDTH-1:0] pdoA,pdoB;
wire              porA,porB;
wire              prdA,prdB;
wire              selA,selB;
reg               Last;

A2_pipe #(WIDTH) ipA (
   .pdo  (pdoA),     // out
   .por  (porA),     // out
   .prd  (prdA),     // in

   .pdi  (pdiA),     // in
   .pwr  (pwrA),     // in
   .pir  (pirA),     // out

   .clock(clock),
   .reset(reset)
   );


A2_pipe #(WIDTH) ipB (
   .pdo  (pdoB),     // out
   .por  (porB),     // out
   .prd  (prdB),     // in

   .pdi  (pdiB),     // in
   .pwr  (pwrB),     // in
   .pir  (pirB),     // out

   .clock(clock),
   .reset(reset)
   );

assign
   selA           =  porA  & (~Last |  ~porB),     // Arbitrate between input pipes
   selB           =  porB  & ( Last |  ~porA),     // Arbitrate between input pipes
//   psel           =  Last,                         // Output state //JLPL - 10_9_25
   psel           =  selB,                         // Output state //JLPL - 10_9_25
   por            =  porA  |  porB,                // If any, proceed
   pdo[WIDTH-1:0] = {WIDTH {  selA }} & pdoA       // Multiplex between pipes
                  | {WIDTH {  selB }} & pdoB,
   prdA           =  prd   &  selA,                // \___ read from selected pipe
   prdB           =  prd   &  selB;                // /

// Arbiter: Alternating priority
if (ARB_TYPE == "AoverB") begin : AoB
   //always @* Last = 1'b0; //JLPL - 7_30_25 - Set to act on reset for simulation event
   always @* Last = reset; //JLPL - 7_30_25 - Set to act on reset for simulation event
   end
else if (ARB_TYPE == "BoverA") begin : BoA
   //always @* Last = 1'b1; //JLPL - 7_30_25 - Set to act on reset for simulation event
   always @* Last = ~reset; //JLPL - 7_30_25 - Set to act on reset for simulation event   
   end
else begin : ALTERNATING
   always @(posedge clock)
      if      (reset)         Last  <= `tCQ  0;
      else if (prd &&  por)   Last  <= `tCQ  selA;
   end

endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
