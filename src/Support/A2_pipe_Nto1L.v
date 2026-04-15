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
//    Project Name   : Pipeline N to 1 reduction buffer
//
//    Author         : RTT
//    Creation Date  : 5/26/2004
//
//    Description    : Stallable pipestage
//                     This is a pipestage module that when linked in a chain creates a FIFO that may stall in any stage
//                     This can be used as a FIFO or as a stallable pipeline with logic between each stage.
//                     This module reduces an incoming width by a factor of N (i.e. if N=4: 32-bits in -> 8-bits out)
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

module A2_pipe_Nto1L #(
   parameter   WIDTH    =  8,       // Width of OUTPUT
   parameter   FACTOR   =  8,       // Compression degree -- Input width is "FACTOR" times "WIDTH"
   parameter   ENDIAN   =  1        // 0: little endian  1: Big endian
   )(
   output wire        [WIDTH-1:0]   pdo,     // Data Out
   output wire                      por,     // Output Ready
   output wire                      plast,   // Last output of an input group
   input  wire                      prd,     // Read        -- FIFO pop

   output wire                      pir,     // Input Ready
   input  wire [FACTOR*WIDTH-1:0]   pdi,     // Data In
   input  wire                      pwr,     // Write       -- FIFO push

   input  wire                      clock,
   input  wire                      reset
   );

localparam  W  = WIDTH;
localparam  E  = FACTOR * WIDTH;
localparam  LW = 3;                 // log 2 of FACTOR
localparam  L  = 1'b 0;
localparam  H  = 1'b 1;
localparam  x  = 1'b x;

reg   [E-1:0] pipe;         // Pipeline register normally one deep
reg   [E-1:0] skid;         // Slave register for two deep
reg   [LW :0] count;        // Count of available words for output //Joel 2_7_25 -> Compiler failed localparam above
reg   [LW :0] tally;        // Count of words output modulo FACTOR
reg           ld_skid, ld_pipe, sel_data, sel_shft, sel_skid, inc_byF, dec_by1, inc_byFm1;

wire  [E-1:0] shft;

// Pipeline Registers -----------------------------------------------------------------------------------------------------------

generate
   if (ENDIAN) begin : A
      assign   shft[E-1:0] =  {pipe[E-W-1:0], pipe[E-1:E-W]};
      end
   else begin : B
      assign   shft[E-1:0] =  {pipe[W-1:0], pipe[E-1:W]};
      end
   endgenerate

always @(posedge clock) begin
   if ( ld_skid )
      skid[E-1:0]  <= `tCQ  pdi[E-1:0];
   if ( ld_pipe ) begin
      if ( sel_data )   begin pipe[E-1:0]  <= `tCQ  pdi[E-1:0];   tally <= #1 FACTOR-1;   end
      if ( sel_shft )   begin pipe[E-1:0]  <= `tCQ  shft[E-1:0];  tally <= #1 tally -1'b1;end
      if ( sel_skid )   begin pipe[E-1:0]  <= `tCQ  skid[E-1:0];  tally <= #1 FACTOR-1;   end
      end
   end

assign   plast = tally == {LW+1{1'b0}};

// State Machine ----------------------------------------------------------------------------------------------------------------

always @* begin
   if ( count[LW:0] == 0 ) //Joel 2_7_25
      casez ({pwr, prd})
`define TT  {ld_skid, ld_pipe, sel_data, sel_shft, sel_skid, inc_byF, dec_by1, inc_byFm1}
//                  \    \       |      / ________/         /        /        /
//                   \    \      |     / /  _______________/        /        /
//                    \    \     |    / /  / ______________________/        /
//                     \___ \    |   / /  / / _____________________________/
//                         \ \   |  / /  / / /
         2'b 1_? : `TT = {  L,H, H,L,L, H,L,L }; //Joel 2_7_25 -> Set x conditions to known state
         default : `TT = {  L,L, L,L,L, L,L,L }; //Joel 2_7_25 -> Set x conditions to known state
         endcase
   else if ( count[LW:0] == {{LW{1'b 0}},1'b 1}) //Joel 2_7_25
      casez ({pwr, prd})
         2'b 1_1 : `TT = {  L,H, H,L,L, L,L,H }; //Joel 2_7_25 -> Set x condition for ld_skid to known state
         2'b 1_0 : `TT = {  H,L, x,x,x, H,L,L };
         2'b 0_1 : `TT = {  L,L, x,x,x, L,H,L }; //Joel 2_7_25 -> Set x condition for ld_skid to known state
         default : `TT = {  L,L, x,x,x, L,L,L }; //Joel 2_7_25 -> Set x condition for ld_skid to known state
         endcase
   else if ( count[LW:0] <  FACTOR[LW:0]) //Joel 2_7_25
      casez ({pwr, prd})
         2'b 1_1 : `TT = {  H,H, L,H,L, L,L,H };
         2'b 1_0 : `TT = {  H,L, x,x,x, H,L,L };
         2'b 0_1 : `TT = {  L,H, L,H,L, L,H,L };
         default : `TT = {  L,L, L,L,L, L,L,L }; //Joel 2_7_25 -> Set x conditions to known state
         endcase
   else if ( count[LW:0] == FACTOR[LW:0]) //Joel 2_7_25
      casez ({pwr, prd})
         2'b 1_1 : `TT = {  H,H, L,H,L, L,L,H };
         2'b 1_0 : `TT = {  H,L, x,x,x, H,L,L };
         2'b 0_1 : `TT = {  L,H, L,H,L, L,H,L };
         default : `TT = {  L,L, L,L,L, L,L,L }; //Joel 2_7_25 -> Set x conditions to known state
         endcase
   else if ( count[LW:0] == (FACTOR[LW:0]+1'b 1)) //Joel 2_7_25
      casez ({pwr, prd})
         2'b ?_1 : `TT = {  L,H, L,L,H, L,H,L };
         default : `TT = {  L,L, L,L,L, L,L,L }; //Joel 2_7_25 -> Set x conditions to known state
         endcase
   else if ( count[LW:0] >  (FACTOR[LW:0]+1'b 1)) //Joel 2_7_25
         casez ({pwr, prd})
         2'b ?_1 : `TT = {  L,H, L,H,L, L,H,L };
         default : `TT = {  L,L, L,L,L, L,L,L }; //Joel 2_7_25 -> Set x conditions to known state
         endcase
   else            `TT = {  L,L, L,L,L, L,L,L }; //Joel 2_7_25 -> Set x conditions to known state
   end

always @(posedge clock)
   if (reset)           count[LW:0] <= `tCQ  {LW+1{1'b 0}}; //Joel 2_7_25
   else begin
      if ( inc_byF   )  count[LW:0] <= `tCQ  count[LW:0] + FACTOR[LW:0];         //Joel 2_7_25
      if ( dec_by1   )  count[LW:0] <= `tCQ  count[LW:0] - 1'b1;                 //Joel 2_7_25
      if ( inc_byFm1 )  count[LW:0] <= `tCQ  count[LW:0] + FACTOR[LW:0] -1'b 1;  //Joel 2_7_25
      end

// Outputs ---------------------------------------------------------------------------------------------------------------------

generate
   if (ENDIAN) begin : C
      assign   pdo[W-1:0]  =  pipe[E-1:E-W];
      end
   else begin : D
      assign   pdo[W-1:0]  =  pipe[W-1:0];
      end
   endgenerate

assign   por    = ~(count[LW:0] == {LW+1{1'b 0}}); //Joel 2_7_25
assign   pir    =  (count[LW:0] <=  FACTOR[LW:0]); //Joel 2_7_25

endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

// Set a command line `define to "verify_A2_Nto1buf" to do a standalone module test on this file

`ifdef   verify_A2_Nto1buf

module t;

parameter   WIDTH       =  8;
parameter   FACTOR      = 45;
parameter   TICK        = 10;

reg  [FACTOR*WIDTH-1:0] pdi;
reg                     pwr;       // Like a FIFO push (active)
reg            prd;       // Like a FIFO pop  (active)

wire [WIDTH-1:0] pdo;
wire           por;       // Like a FIFO not empty (Output Ready)
wire           pir;       // Like a FIFO not full (Input Ready)

reg            clock;
reg            reset;
integer        i;

A2_Nto1buf  #(
   .WIDTH   (WIDTH),
   .FACTOR  (FACTOR),
   .ENDIAN  (1)
   ) mut (
   .pdi     (pdi),
   .pwr     (pwr),       // Like a FIFO push (active)
   .pir     (pir),       // Like a FIFO not full (Input Ready)

   .pdo     (pdo),
   .por     (por),       // Like a FIFO not empty (Output Ready)
   .prd     (prd),        // Like a FIFO pop  (active)

   .clock   (clock),
   .reset   (reset)
   );


initial begin
   #1;
   clock     = 1;
   forever begin
      #(TICK/2) clock = 0;
      #(TICK/2) clock = 1;
      end
   end

reg  [FACTOR*WIDTH-1:0] testdata;

always @(posedge clock) begin
   if (reset) begin
      for (i=0; i<FACTOR; i=i+1) begin
         testdata[WIDTH*i+:WIDTH] = i;
         end
      #1;
      pdi      =  testdata;
      end
   else if (pwr && pir) begin
      for (i=0; i<FACTOR; i=i+1) begin
         testdata[WIDTH*i+:WIDTH] = testdata[WIDTH*i+:WIDTH] + 1;
         end
      #1;
      pdi   =  testdata;
      end
   end

initial begin
      pdi   =  0;
      pwr =  0;
      prd  =  0;
      reset    =  0;
   @(posedge clock);
   #1             reset    =  1;
   #(TICK * 3);   reset    =  0;
   #(TICK * 2);   pwr =  1;
   #(TICK);       pwr =  0;
   @(posedge clock);

   repeat (FACTOR) begin
                  prd  =  0;
      #(TICK);    prd  =  1;
      #(TICK);
      end
   #(TICK*2);     pwr =  1;
                  prd  =  1;

   repeat (2* FACTOR) @(posedge clock);
                  pwr =  0;

   repeat (2* FACTOR) @(posedge clock);
                  prd  =  0;

   #(TICK*100);

   $finish;


   end

endmodule

`endif

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

