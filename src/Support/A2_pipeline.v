//===============================================================================================================================
//
//    Copyright © 2021..2025 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to Authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//    Project Name         : Cognitum
//
//    Description          : core of 256 processor array
//
//===============================================================================================================================
//    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//
//    NOTES:
//
//===============================================================================================================================

module   A2_pipeline #(
   parameter   WIDTH    = 9,
   parameter   DEPTH    = 4
   )(
   input  wire [WIDTH-1:0] pdi,
   input  wire             pwr,
   output wire             pir,
   input  wire [WIDTH-1:0] pfm,     // Forcemeat:  Value to compare against; to trigger 'force' action.

   output wire [WIDTH-1:0] pdo,
   output wire             por,
   input  wire             prd,

   input  wire             cki,
   input  wire             rsi,
   output wire             cko,
   output wire             rso
   );

// Pipeline =====================================================================================================================

//     Width            pipe-depth
wire  [WIDTH-1:0] data  [0:DEPTH];
wire              give  [0:DEPTH];
wire              take  [0:DEPTH];
wire              urge  [0:DEPTH];        // force is a reserved word in verilog so 'urge' is used instead!
wire              clock [0:DEPTH];
wire              reset [0:DEPTH];

assign
   clock[0]    =  cki,
   reset[0]    =  rsi,
   data[0]     =  pdi,
   give[0]     =  pwr,
   take[DEPTH] =  prd,
   pdo         =  data[DEPTH],
   por         =  give[DEPTH],
   pir         =  take[0],
   cko         =  clock[DEPTH],
   rso         =  reset[DEPTH];

genvar i;

for (i=0; i<DEPTH; i=i+1) begin : B
   assign urge[i] = data[i] == pfm;       //            :               :               :               :
   A2_pipe_force #(WIDTH) iPF (           //    i=0     :      i=1      :      i=2      :      i=3      :     i=4
      .por    (give[i+1]),                //          PIPE-0          PIPE-1          PIPE-2          PIPE-3              //JLPL
      .pdo    (data[i+1][8:0]),           //         +-----+         +-----+         +-----+         +-----+              //JLPL
      .prd    (take[i+1]),                //  =data=>|di do|===[1]==>|di do|===[2]==>|di do|===[3]==>|di do|==data[4]=>
      .pfc    (urge[i  ]),                //  -urge->|fc   |--urge-->|fc   |--urge-->|fc or|--urge-->|fc   |
      .pir    (take[i  ]),                // --give->|wr or|--give-->|wr or|--give-->|wr or|--give-->|wr or|--give[4]->
      .pdi    (data[i  ][8:0]),           //         |     |         |     |         |     |         |     |
      .pwr    (give[i  ]),                //         |     |         |     |         |     |         |     |
      .clock  (clock[i ]),                // <-take--|ir rd|<-take---|ir rd|<-take---|ir rd|<-take---|ir rd|<-take[4]--
      .reset  (reset[i ])                 //         +--^--+         +--^--+         +--^--+         +--^--+
      );                                  // clock _____|_______________|_______________|_______________|__________=>
   assign clock[i+1] = clock[i];          // & reset    :               :               :               :
   assign reset[i+1] = reset[i];          //            :               :               :               :
   end

endmodule

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

`ifdef   verify_A2_pipeline
module t;

parameter   WIDTH = 9;
parameter   DEPTH = 4;

// Pipeline Input
reg   [WIDTH-1:0] pdi;
reg   [WIDTH-1:0] pfm;
reg               pwr;
reg               cki;
reg               rsi;
wire              pir;
// Pipeline Output
wire  [WIDTH-1:0] pdo;
wire              por;
reg               prd;
wire              cko;
wire              rso;

A2_pipeline #(
   .WIDTH(WIDTH),
   .DEPTH(DEPTH)
   ) t (
   .pdi  (pdi),
   .pwr  (pwr),
   .pir  (pir),
   .pdo  (pdo),
   .por  (por),
   .prd  (prd),
   .pfm  (pfm),
   .cki  (cki),
   .rsi  (rsi),
   .cko  (cko),
   .rso  (rso)
   );

initial begin
   cki   = 1'b0;
   forever
      cki   = #10 ~cki  ;
   end

initial begin
   rsi      = 0;
   repeat (10) @(posedge cki) #1;
   rsi      = 1;
   repeat (10) @(posedge cki) #1;
   rsi      = 0;
   repeat (10) @(posedge cki) #1;
   while(!pir) @(posedge cki)
   #1;
   prd      = 0;
   pwr      = 1;
   pdi      = 9'h011;
   repeat (1)  @(posedge cki) #1;
   pdi      = 9'h022;
   repeat (1)  @(posedge cki) #1;
   pdi      = 9'h033;
   repeat (1)  @(posedge cki) #1;
   pdi      = 9'h044;
   repeat (1)  @(posedge cki) #1;
   pwr      = 0;
   while(!por) @(posedge cki)
   #1;
   if (pdo != 9'h011) $display(" ERROR:  Expected %h  found %h", 9'h011, pdo);
   prd      = 1;
   repeat (4)  @(posedge cki) #1;
   prd      = 0;



   repeat (100)  @(posedge cki) #1;
   $finish();
   end

endmodule
`endif
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
