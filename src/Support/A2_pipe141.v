//===============================================================================================================================
//
//    Copyright © 1996..2023 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//
//    Project Name   : Pipeline
//
//    Description    : Stallable pipestage 1-4-1
//                     This is a pipestage module that when linked in a chain creates a FIFO that may stall in any stage
//                     This can be used as a FIFO or as a stallable pipeline with logic between each stage.
//                     This version uses a FACTOR "N" that sets the overall size.  There must "N" input pushes before and output
//                     is available. So, for example, if N is 4 there must be 4 pushes before data is available on the output.
//                     Rather like a pipe1to4 back-to-back with a pipe4to1 but without skid buffers as pipe1to4 looks like a skid
//                     buffer.
//                     It is built from shift registers
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

module A2_pipe_141 #(
   parameter    WIDTH   =  8
   )(
   // Output Interface
   output wire [WIDTH-1:0] pdo,           // Output data
   output wire             por,           // Like a FIFO not empty (Output Ready)
   input  wire             prd,           // Like a FIFO pop       (read)
   // Input Interface
   input  wire [WIDTH-1:0] pdi,           // Input data
   input  wire             pwr,           // Like a FIFO push      (write)
   output wire             pir,           // Like a FIFO not full  (Input Ready)

   input  wire             clock,
   input  wire             reset
   );


reg   [4*WIDTH-1:0]  Skid, Pipe;
reg           [2:0]  ISM, OSM;            // Input and Output Finite FSM Machines

wire                 mov;

//Joel 3_2_25 - Move logic up for compile error of use before assignment
localparam  [2:0] MPT   =  0,             // stage is empty
                  UNO   =  1,             // stage has one word
                  DOS   =  2,             // stage has two words
                  TRE   =  3,             // stage has three words
                  FAW   =  4;             // stage has four words

wire     ifaw  =  ISM == FAW,
         ompt  =  OSM == MPT,
         ouno  =  OSM == UNO;
//End Joel 3_2_25

always @(posedge clock)  if ( pir && pwr )   Skid[4*WIDTH-1:0] <= `tCQ {pdi[WIDTH-1:0], Skid[4*WIDTH-1:WIDTH]};

always @(posedge clock)  if ( mov        )   Pipe[4*WIDTH-1:0] <= `tCQ Skid[4*WIDTH-1:0]; //Per Roger
                    else if ( por && prd )   Pipe[4*WIDTH-1:0] <= `tCQ Pipe[4*WIDTH-1:0] >> WIDTH;

//localparam  [2:0] MPT   =  0,             // stage is empty        //Joel 3_2_25
//                  UNO   =  1,             // stage has one word    //Joel 3_2_25
//                  DOS   =  2,             // stage has two words   //Joel 3_2_25
//                  TRE   =  3,             // stage has three words //Joel 3_2_25
//                  FAW   =  4;             // stage has four words  //Joel 3_2_25

always @(posedge clock)
   if (reset)                  ISM[2:0] <= `tCQ MPT;
   else case (ISM)
      MPT : if ( pwr         ) ISM[2:0] <= `tCQ UNO;
      UNO : if ( pwr         ) ISM[2:0] <= `tCQ DOS;
      DOS : if ( pwr         ) ISM[2:0] <= `tCQ TRE;
      TRE : if ( pwr         ) ISM[2:0] <= `tCQ FAW;
      FAW : if ( pwr &&  mov ) ISM[2:0] <= `tCQ UNO;
       else if (!pwr &&  mov ) ISM[2:0] <= `tCQ MPT;
      endcase

always @(posedge clock)
   if (reset)                  OSM[2:0] <= `tCQ MPT;
   else case (OSM)
      MPT : if (         mov  ) OSM[2:0] <= `tCQ FAW;
      FAW : if ( prd          ) OSM[2:0] <= `tCQ TRE;
      TRE : if ( prd          ) OSM[2:0] <= `tCQ DOS;
      DOS : if ( prd          ) OSM[2:0] <= `tCQ UNO;
      UNO : if ( prd &&  ifaw ) OSM[2:0] <= `tCQ FAW; //Per Roger
       else if ( prd          ) OSM[2:0] <= `tCQ MPT; //Per Roger
      endcase

//wire     ifaw  =  ISM == FAW, //Joel 3_2_25
//         ompt  =  OSM == MPT, //Joel 3_2_25
//         ouno  =  OSM == UNO; //Joel 3_2_25

assign   mov   =  ifaw & ompt
               |  ifaw & ouno & prd;

assign   pir   =  ISM != FAW;
assign   por   =  OSM != MPT;

assign   pdo[WIDTH-1:0] =  Pipe[WIDTH-1:0];

endmodule



////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

// Set a command line `define to "verify_A2_pipe141" to do a standalone module test on this file

`ifdef   verify_A2_pipe

module t;

parameter   TICK  = 250;
parameter   WIDTH =   8;

reg   [WIDTH-1:0] datai;
reg               write;
reg               read;
reg               reset;
reg               clock;

wire  [WIDTH-1:0] datao;
wire              ordy, irdy;


////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

A2_pipe141 #(WIDTH) p0 (
   .pdo  (datao),                 // out
   .por  (ordy ),                 // out
   .pir  (irdy ),                 // out
   .pdi  (datai),                 // in
   .pwr  (write),                 // in
   .prd  (read ),                 // in
   .clock(clock),
   .reset(reset)
   );

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

integer  errors;

initial begin
   clock =  1'b1;
   forever  #(TICK/2)   clock = ~clock;
   end

initial begin
   errors   = 0;
   reset    = 1'b0;
   repeat (10) @(posedge clock); #1
   reset    = 1'b1;
   repeat (10) @(posedge clock); #1
   reset    = 1'b0;
   datai    = 8'h11;
   read     = 1'b0;
   write    = 1'b1;
   repeat (1)  @(posedge clock); #1
   write    = 1'b0;
   repeat (1)  @(posedge clock); #1
   write    = 1'b1;
   datai    = 8'h22;
   repeat (1)  @(posedge clock); #1
   write    = 1'b0;
   repeat (1)  @(posedge clock); #1
   write    = 1'b1;
   datai    = 8'h33;
   repeat (1)  @(posedge clock); #1
   write    = 1'b0;
   repeat (1)  @(posedge clock); #1
   write    = 1'b1;
   datai    = 8'h44;
   read     = 1'b1;
   repeat (1)  @(posedge clock); #1
   write    = 1'b0;
   repeat (1)  @(posedge clock); #1
   read     = 1'b1;
   repeat (1)  @(posedge clock); #1
   read     = 1'b0;
   repeat (1)  @(posedge clock); #1
   read     = 1'b1;
   repeat (1)  @(posedge clock); #1
   read     = 1'b0;
   repeat (1)  @(posedge clock); #1
   read     = 1'b1;
   repeat (1)  @(posedge clock); #1
   read     = 1'b0;
   repeat (1)  @(posedge clock); #1
   read     = 1'b1;
   repeat (1)  @(posedge clock); #1
   read     = 1'b0;
   repeat (40) @(posedge clock); #1
   $display("\n TEST COMPLETED WITH %2d ERRORS \n",errors);
   $stop();
   $finish();
   end

endmodule

`endif

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

