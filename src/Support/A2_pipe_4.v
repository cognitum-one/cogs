//===============================================================================================================================
//
//    Copyright © 2004..2024 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//
//    Project Name   : Pipeline: 4 deep, one pipe with 3 skid buffers
//
//    Author         : RTT
//    Creation Date  : 5/26/2004
//
//    Description    : Stallable pipestage
//                     This is a pipestage module that when linked in a chain creates a FIFO that may stall in any stage
//                     This can be used as a FIFO or as a stallable pipeline with logic between external stages.
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

//`define  tCQ   #1

module A2_pipe_4 #(
   parameter  WIDTH = 8
   )(
   input  [WIDTH-1:0] pdi,
   input              pwr,     // Like a FIFO push
   output             pir,     // Like a FIFO not full  (Input  Ready)

   output [WIDTH-1:0] pdo,
   output             por,     // Like a FIFO not empty (Output Ready)
   input              prd,     // Like a FIFO pop

   input              clock,
   input              reset
   );

reg   [2:0]       FSM;   localparam  [2:0] MPT = 3'h0,  ST1 = 3'h1,  ST2 = 3'h2,  ST3 = 3'h3,  ST4 = 3'h4;
reg   [WIDTH-1:0] PIPE0, SKID1, SKID2, SKID3;

always @(posedge clock) begin
   if (reset)           FSM   <= `tCQ  MPT;
   else casez ({FSM,pwr,prd})
      {MPT, 2'b 1?}  :  FSM   <= `tCQ ST1;
      {ST1, 2'b 01}  :  FSM   <= `tCQ MPT;
      {ST1, 2'b 10}  :  FSM   <= `tCQ ST2;
      {ST2, 2'b 01}  :  FSM   <= `tCQ ST1;
      {ST2, 2'b 10}  :  FSM   <= `tCQ ST3;
      {ST3, 2'b 01}  :  FSM   <= `tCQ ST2;
      {ST3, 2'b 10}  :  FSM   <= `tCQ ST4;
      {ST4, 2'b ?1}  :  FSM   <= `tCQ ST3;
      default           FSM   <= `tCQ FSM;
      endcase
   end

wire     empty    =  FSM == MPT,
         one      =  FSM == ST1,
         two      =  FSM == ST2,
         three    =  FSM == ST3,
         full     =  FSM == ST4;

wire     ld_pipe0 =  empty &  pwr
                  |  one   &  pwr &  prd
                  |  two   &         prd
                  |  three &         prd
                  |  full  &         prd;

wire     ld_skid1 =  one   &  pwr & ~prd
                  |  two   &  pwr &  prd
                  |  three &         prd
                  |  full  &         prd;

wire     ld_skid2 =  two   &  pwr & ~prd
                  |  three &  pwr &  prd
                  |  full  &         prd;

wire     ld_skid3 =  three &  pwr & ~prd
                  |  full  &  pwr &  prd;

wire     sel_1    =  empty |  one,
         sel_2    =  one   |  two,
         sel_3    =  two   |  three;

always @(posedge clock) if (ld_pipe0)  PIPE0  <= `tCQ  sel_1 ? pdi  : SKID1;
always @(posedge clock) if (ld_skid1)  SKID1  <= `tCQ  sel_2 ? pdi  : SKID2;
always @(posedge clock) if (ld_skid2)  SKID2  <= `tCQ  sel_3 ? pdi  : SKID3;
always @(posedge clock) if (ld_skid3)  SKID3  <= `tCQ          pdi;

assign   pdo   =  PIPE0,
         por   = ~empty,
         pir   = ~full;

endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Set a command line `define to "verify_A2_pipe4" to do a standalone module test on this file /////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

`ifdef   verify_A2_pipe4

module t;

parameter   TICK  = 10;
parameter   WIDTH =  8;

reg   [WIDTH-1:0] di;
reg               push;
reg               pop;

wire  [WIDTH-1:0] do;
wire              irdy;
wire              ordy;

reg               clock;
reg               reset;

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

A2_pipe4 mut (
   .pdi  (di   ),    // in
   .pwr  (push ),    // in
   .pir  (irdy ),    // out

   .pdo  (do   ),    // out
   .por  (ordy ),    // out
   .prd  (pop  ),    // in

   .clock(clock),
   .reset(reset)
   );

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

initial begin
   clock    = 0;
   forever
      #(TICK/2) clock = ~clock;
   end

initial begin
   reset    =  0;
   push     =  0;
   pop      =  0;
   reset    =  1;
   repeat (2)  @(posedge clock); #1;
   reset    =  0;
   repeat (5)  @(posedge clock); #1;

   // try some simple pushes and pops  depth of one

   // Overlap by one
   di       =  1;
   push     =  1;
   @(posedge clock); #1;
   pop      =  1;
   di       = di +1;
   @(posedge clock); #1;
   di       = di +1;
   @(posedge clock); #1;
   di       = di +1;
   @(posedge clock); #1;
   push     =  0;
   @(posedge clock); #1;
   pop      =  0;
   @(posedge clock); #1;

   // Overlap by two
   di       =  1;
   push     =  1;
   @(posedge clock); #1;
   di       = di +1;
   @(posedge clock); #1;
   pop      =  1;
   di       = di +1;
   @(posedge clock); #1;
   di       = di +1;
   @(posedge clock); #1;
   push     =  0;
   @(posedge clock); #1;
   @(posedge clock); #1;
   pop      =  0;
   @(posedge clock); #1;

   // Overlap by three
   di       =  1;
   push     =  1;
   @(posedge clock); #1;
   di       = di +1;
   @(posedge clock); #1;
   di       = di +1;
   @(posedge clock); #1;
   pop      =  1;
   di       = di +1;
   @(posedge clock); #1;
   push     =  0;
   @(posedge clock); #1;
   @(posedge clock); #1;
   @(posedge clock); #1;
   pop      =  0;
   @(posedge clock); #1;

   // Overlap by four
   di       =  1;
   push     =  1;
   @(posedge clock); #1;
   di       = di +1;
   @(posedge clock); #1;
   di       = di +1;
   @(posedge clock); #1;
   di       = di +1;
   @(posedge clock); #1;
   pop      =  1;
   push     =  0;
   @(posedge clock); #1;
   @(posedge clock); #1;
   @(posedge clock); #1;
   @(posedge clock); #1;
   pop      =  0;
   @(posedge clock); #1;

   // Hold at two
   di       =  1;
   push     =  1;
   @(posedge clock); #1;
   di       = di +1;
   @(posedge clock); #1;
   di       = di +1;
   pop      =  1;
   @(posedge clock); #1;
   di       = di +1;
   @(posedge clock); #1;
   di       = di +1;
   @(posedge clock); #1;
   di       = di +1;
   @(posedge clock); #1;
   di       = di +1;
   @(posedge clock); #1;
   push     =  0;
   @(posedge clock); #1;
   @(posedge clock); #1;
   pop      =  0;
   @(posedge clock); #1;

   $display("\n TEST COMPLETED \n");
   $stop();
   $finish();
   end

endmodule

`endif

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////



