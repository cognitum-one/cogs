//===============================================================================================================================
//
//    Copyright © 2004..2025 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//
//    Project Name   : Pipeline  with prioritized output
//
//    Author         : RTT
//    Creation Date  : 5/26/2004
//
//    Description    : Stallable pipestage
//                     This is a pipestage module that when linked in a chain creates a FIFO that may stall in any stage
//                     This can be used as a FIFO or as a stallable Pipe0line with logic between each stage.
//                     This is a pipestage (based on A2_pipe_priority3.4.v) that blocks swapping if a Broadcast request is the active state in Pipe
//
//===============================================================================================================================
//    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//==============================================================================================================================
//
//    THIS VARIANT USES THE MSB OF THE DATA TO FLAG PRIORITY.  1 IS LOW AND 0 IS HIGH.
//
//    NOTA BENE:  Only one low priority access is allowed in the Pipe at any one time!!
//===============================================================================================================================

`ifdef synthesis //JLPL
	 `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh"
`else
`include "A2_project_settings.vh"
`endif
//End JLPL
//`ifndef  tCQ //JLPL
//`define  tCQ   #1 //JLPL
//`endif //JLPL

module A2_pipe_priority_broadcast #(
   parameter    WIDTH   =  8
   )(
   // Output Interface
   output wire              por,          // Like a FIFO not empty (Low  Priority Output Ready)
   output wire [WIDTH-1:0]  pdo,          // Output data           (Pipe Data Out)
   input  wire              prd,          // Like a FIFO pop       (Low  Priority Pipe Read)
   // Input Interface
   output wire              pir,          // Like a FIFO not full  (Low  Priority Input Ready)
   input  wire [WIDTH-1:0]  pdi,          // Input data            (Pipe Data In
   input  wire              pwr,          // Like a FIFO push      (High priority Write)

   input  wire            clock,
   input  wire            reset
   );

localparam  MPT   =  0,                   // Pipestage is empty
            ONE   =  1,                   // Pipestage has one word  (in 'pipe')
            TWO   =  2,                   // Pipestage has two words (one in 'skid' one in 'pipe')
            LOW   =  3;                   // Pipestage has one low priority

localparam    L   =  1'b0,  H = 1'b1,  x = 1'b?;

reg   [WIDTH-1:0] Pipe;                   // Pipeline register normally one deep
reg   [WIDTH-1:0] Skid;                   // Slave register for two deep
reg         [3:0] FSM;                    // Finite FSM Machine

wire  Phi   =  !Pipe[WIDTH-1],            // Priority of data  in Pipe is high  i.e. WITDH-1 bit is Low!
      Shi   =  !Skid[WIDTH-1],            // Priority of data  in Skid is high
	  PBcast =  Pipe[WIDTH-3],            // Pipe is holding a Broadcast Request
      Ihi   =  !pdi [WIDTH-1];            // Incoming data is high priority

integer     caseF;                        // For debug only

always @(posedge clock) begin
   if (reset)                    FSM              <= `tCQ  MPT;
   else casez ({FSM[1:0],pwr,prd,Phi,Shi,Ihi,PBcast})
      //       / ________/   /   /   /   /   /  
      //      / / __________/   /   /   /   /
      //     / / / ____________/   /   /   /
      //    / / / / ______________/   /   /
      //   / / / / / ________________/   /
      //  / / / / / /  ________________ / 
	  // / / / / / /  /
      {MPT,L,x,x,x,x,x} : begin { FSM, Pipe, Skid } <= `tCQ {MPT, Pipe, Skid }; caseF <= 8'd00; end // No Action //JLSW
      {MPT,H,x,x,x,L,x} : begin { FSM, Pipe, Skid } <= `tCQ {LOW, pdi,  Skid }; caseF <= 8'd01; end // Can't read 'por' is OFF //JLSW
      {MPT,H,x,x,x,H,x} : begin { FSM, Pipe, Skid } <= `tCQ {ONE, pdi,  Skid }; caseF <= 8'd02; end // Can't read 'por' is OFF //JLSW
      //-------------------------------------------------------------------------------------------------------------------------
      {LOW,L,L,x,x,x,x} : begin { FSM, Pipe, Skid } <= `tCQ {LOW, Pipe, Skid }; caseF <= 8'd03; end // Nothing Happening stay put //JLSW
      {LOW,L,H,x,x,x,x} : begin { FSM, Pipe, Skid } <= `tCQ {MPT, Pipe, Skid }; caseF <= 8'd04; end // Go back to Empty //JLSW
      {LOW,H,L,x,x,x,x} : begin { FSM, Pipe, Skid } <= `tCQ {ONE, Pipe, Skid }; caseF <= 8'd05; end // Write rejected goto One //JLSW
      {LOW,H,H,x,x,x,x} : begin { FSM, Pipe, Skid } <= `tCQ {MPT, Pipe, Skid }; caseF <= 8'd06; end // Write rejected, Read Accepted //JLSW
      //-------------------------------------------------------------------------------------------------------------------------
      {ONE,L,L,x,x,x,x} : begin { FSM, Pipe, Skid } <= `tCQ {ONE, Pipe, Skid }; caseF <= 8'd07; end // Nada //JLSW
      {ONE,L,H,x,x,x,x} : begin { FSM, Pipe, Skid } <= `tCQ {MPT, Pipe, Skid }; caseF <= 8'd08; end // Read -> Empty //JLSW
      {ONE,H,L,L,x,L,x} : begin { FSM, Pipe, Skid } <= `tCQ {ONE, Pipe, Skid }; caseF <= 8'd09; end // Reject write -> force pir low //JLSW
      {ONE,H,L,L,x,H,L} : begin { FSM, Pipe, Skid } <= `tCQ {TWO, pdi,  Pipe }; caseF <= 8'd10; end // Broadcast Request not in Pipe, Pipe is low; move pipe -> Skid, new -> Pipe //JLSW
      {ONE,H,L,L,x,H,H} : begin { FSM, Pipe, Skid } <= `tCQ {TWO, Pipe,  pdi }; caseF <= 8'd30; end // Broadcast Request in Pipe, Write Input to Skid //JLSW	  
      {ONE,H,L,H,x,x,x} : begin { FSM, Pipe, Skid } <= `tCQ {TWO, Pipe, pdi  }; caseF <= 8'd11; end // Pipe already high so Skid //JLSW
      {ONE,H,H,L,x,L,x} : begin { FSM, Pipe, Skid } <= `tCQ {MPT, Pipe, Skid }; caseF <= 8'd12; end // Read but reject write //JLSW
      {ONE,H,H,H,x,L,x} : begin { FSM, Pipe, Skid } <= `tCQ {ONE, pdi,  Skid }; caseF <= 8'd13; end // Read & Reload Pipe //JLSW
      {ONE,H,H,x,x,H,x} : begin { FSM, Pipe, Skid } <= `tCQ {ONE, pdi,  Skid }; caseF <= 8'd14; end // Read & Reload Pipe //JLSW
      //-------------------------------------------------------------------------------------------------------------------------
      {TWO,L,L,L,L,x,x} : begin { FSM, Pipe, Skid } <= `tCQ {TWO, Pipe, Skid }; caseF <= 8'd15; end // Illegal //JLSW
      {TWO,L,L,H,L,x,x} : begin { FSM, Pipe, Skid } <= `tCQ {TWO, Pipe, Skid }; caseF <= 8'd16; end // Pipe already High Priority; leave alone //JLSW
      {TWO,L,L,L,H,x,L} : begin { FSM, Pipe, Skid } <= `tCQ {TWO, Skid, Pipe }; caseF <= 8'd17; end // Broadcast Request not in Pipe so Swap on ~Phi & Shi //JLSW
      {TWO,L,L,L,H,x,H} : begin { FSM, Pipe, Skid } <= `tCQ {TWO, Pipe, Skid }; caseF <= 8'd37; end // Broadcast Request in Pipe so don't Swap on ~Phi & Shi //JLSW 
      {TWO,L,L,H,H,x,x} : begin { FSM, Pipe, Skid } <= `tCQ {TWO, Pipe, Skid }; caseF <= 8'd18; end // Pipe already High Priority; leave alone //JLSW

      {TWO,L,H,x,x,x,x} : begin { FSM, Pipe, Skid } <= `tCQ {ONE, Skid, Skid }; caseF <= 8'd19; end // Read Pipe; go down a level //JLSW

      {TWO,H,L,L,L,x,x} : begin { FSM, Pipe, Skid } <= `tCQ {TWO, Pipe, Skid }; caseF <= 8'd20; end // Illegal //JLSW
      {TWO,H,L,H,L,x,x} : begin { FSM, Pipe, Skid } <= `tCQ {TWO, Pipe, Skid }; caseF <= 8'd21; end // Pipe already High Priority; leave alone //JLSW
      {TWO,H,L,L,H,x,L} : begin { FSM, Pipe, Skid } <= `tCQ {TWO, Skid, Pipe }; caseF <= 8'd22; end // Broadcast Request not in Pipe so Swap on ~Phi &  Shi //JLSW
      {TWO,H,L,L,H,x,H} : begin { FSM, Pipe, Skid } <= `tCQ {TWO, Pipe, Skid }; caseF <= 8'd32; end // Broadcast Request in Pipe so don't Swap on ~Phi &  Shi //JLSW	  
      {TWO,H,L,H,H,x,x} : begin { FSM, Pipe, Skid } <= `tCQ {TWO, Pipe, Skid }; caseF <= 8'd23; end // Pipe already High Priority; leave alone //JLSW

      {TWO,H,H,x,x,x,x} : begin { FSM, Pipe, Skid } <= `tCQ {ONE, Skid, Skid }; caseF <= 8'd24; end // NO WRITE 'pir' is OFF; capture next cycle! //JLSW
      endcase
   end

assign
   pir            =  (FSM == MPT) | (FSM == ONE) & (Phi | Ihi),    // Timing-wise Ihi is coming from source so should be OK!
   por            = ~(FSM == MPT),
   pdo[WIDTH-1:0] =  Pipe[WIDTH-1:0];

endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

// Set a command line `define to "verify_A2_pipe_priority" for standalone module test on this file

`ifdef   verify_A2_pipe_priority

module t;

parameter   TICK  = 10;
parameter   WIDTH =  8;

integer           errors;
reg  [     31:0]  testval;

// Outputs
wire              por;          // Like a FIFO not empty (Low  Priority Output Ready)
wire [WIDTH-1:0]  pdo;          // Output data           (Pipe Data Out)
wire              pir;          // Like a FIFO not full  (High Priority Input Ready)
// Inputs
reg               prd;          // Like a FIFO pop       (Pipe Read)
reg  [WIDTH-1:0]  pdi;          // Input data            (Pipe Data In
reg               pwr;          // Like a FIFO push      (Write)
reg             clock;
reg             reset;

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

A2_pipe_priority3 #(
   .WIDTH  (WIDTH)
   ) mut (
   // Output Interface
   .por    (por),       // Like a FIFO not empty
   .pdo    (pdo),       // Output data
   .prd    (prd),       // Like a FIFO pop

   .pir    (pir),       // Like a FIFO not full
   .pdi    (pdi),       // Input data
   .pwr    (pwr),       // Like a FIFO push

   .clock  (clock),
   .reset  (reset)
   );

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

initial begin
   clock     = 1;
   forever begin
      #(TICK/2) clock = 0;
      #(TICK/2) clock = 1;
      end
   end

wire [WIDTH-1:0]  data_out = por && prd  ? pdo : {WIDTH{1'bz}};

initial begin
   testval  = "  CLR";
   $display(" RESET ");
   errors   =  0;
   reset    =  0;
   repeat ( 10) @(posedge clock); #1
   reset    =  1;
   repeat ( 10) @(posedge clock); #1
   reset    =  0;
   repeat ( 10) @(posedge clock); #1
   testval  = "  t1";
   $display(" TEST 1 ");
   pdi      =  8'haa;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   prd      =  1;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'hbb;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   prd      =  1;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat ( 10) @(posedge clock); #1
   testval  = "  t2";
   $display(" TEST 2 ");
   pdi      =  8'hcc;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'hdd;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   prd      =  1;
   pwr      =  0;
   repeat (  2) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat ( 10) @(posedge clock); #1
   testval  = "  t3";
   $display(" TEST 3 ");
   pdi      =  8'h6e;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'hff;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   prd      =  1;
   pwr      =  0;
   repeat (  2) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat ( 10) @(posedge clock); #1
   testval  = "  t4";
   $display(" TEST 4 ");
   pdi      =  8'h88;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'h0f;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   prd      =  1;
   pwr      =  0;
   repeat (  2) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat ( 10) @(posedge clock); #1
   testval  = "  t5";
   $display(" TEST 5 ");
   pdi      =  8'h78;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'h0f;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   prd      =  1;
   pwr      =  0;
   repeat (  2) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat ( 10) @(posedge clock); #1
   testval  = "  t6";
   $display(" TEST 6 ");
   pdi      =  8'h88;
   prd      =  1;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'h89;
   prd      =  1;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'h8f;
   prd      =  1;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  1;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat ( 10) @(posedge clock); #1
   testval  = "  t7";
   $display(" TEST 7 ");
   pdi      =  8'h98;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'h99;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'h9f;
   prd      =  1;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  1;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  1;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat ( 10) @(posedge clock); #1
   testval  = "  t8";
   $display(" TEST 8 ");
   pdi      =  8'ha8;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'ha9;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'haf;
   prd      =  1;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  1;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  1;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat ( 10) @(posedge clock); #1
   testval  = "  t9";
   $display(" TEST 9 ");
   pdi      =  8'hf1;
   prd      =  1;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'hf0;
   prd      =  1;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'hf2;
   prd      =  1;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  1;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat ( 10) @(posedge clock); #1
   testval  = " t10";
   $display(" TEST 10 ");
   pdi      =  8'he0;
   prd      =  1;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'he1;
   prd      =  1;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'he2;
   prd      =  1;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'he3;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'he4;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  1;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat ( 10) @(posedge clock); #1
   testval  = " t11";
   $display(" TEST 11 ");
   pdi      =  8'hd0;
   prd      =  1;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'hd1;
   prd      =  1;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'hd2;
   prd      =  1;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'hd3;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   pdi      =  8'h53;
   prd      =  0;
   pwr      =  1;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  1;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  1;
   pwr      =  0;
   repeat (  1) @(posedge clock); #1
   //pdi      =  8'hzz;
   prd      =  0;
   pwr      =  0;
   repeat (100) @(posedge clock); #1
   $display("\n TEST COMPLETED WITH %2d ERRORS \n",errors);
//   $stop();
   $finish();
   end

endmodule

`endif

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

