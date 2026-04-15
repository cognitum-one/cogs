//===============================================================================================================================
//
//    Copyright © 2022 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to Authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//    Project Name         : Cognitum ASIC
//
//    Description          : Quadrant of 8x8 = 64 Tiles with Interconnect to Hub
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
//   NOTES:
//      Four Quadrants 0,1,2 & 3   Quadrant[0] is QuadrantZero and is upper left   [0rrr,0ccc]
//                                 Quadrant[1] is QuadrantOne         upper right  [0rrr,1ccc]
//                                 Quadrant[2] is QuadrantOne         lower left   [1rrr,0ccc]
//                                 Quadrant[3] is QuadrantOne         lower right  [1rrr,1ccc]
//
//      Tile ID =  [cccc,rrrr]     column major   so  cccc = col & rrrr = row
//
//      So quadrant ID [1:0]       msb is Column[3] and lsb is Row[3]
//
//      Column ID needs Full column ID AND msb of Rows from the Quadrant ID.  So column ID is 5 bits; bit 4 is row msb and
//      bit 3 is the column (msb) and bits 2:0 are the generate index below
//
//===============================================================================================================================

`include "A2_project_settings.vh" //JL

module   QuadrantZero (
   // Output to Hub
   output wire [383:0]  QO,    //JL
   output wire [  3:0]  QO_or, //JL
   input  wire [  3:0]  QO_rd, //JL
   output wire          QO_ck, //JL
   // Input from Hub
   input  wire [383:0]  QI,    //JL
   input  wire [  3:0]  QI_wr, //JL
   output wire [  3:0]  QI_ir, //JL
   input  wire          QI_ck, //JL
   // TileZero Interface ----------
   // Output Interface
   output wire [ 97:0]  RT_zero,          // Raceway to TileZero xmosi
   input  wire          RT_zero_fb,
   output wire          RT_zero_ck,
   // Input Interface
   input  wire [ 97:0]  TR_zero,          // TileZero xmiso to Raceway
   output wire          TR_zero_fb,
   input  wire          TR_zero_ck,
   //------------------------------
`ifdef DEBUG
   output wire [ 33:0]  imiso,      // Slave data out {                IOk, IOo[31:0]}
   input  wire [ 47:0]  imosi,      // Slave data in  {cIO[2:0], iIO[31:0], aIO[12:0]}
`endif
   output wire [  3:0]  QPUFosc,
   output wire [ 63:0]  QTIMOo, //JL
   output wire [ 63:0]  FLAGo, //JL
   input  wire [ 63:0]  FLAGi, //JL
   input  wire          reset  //JL
   );

// Nets ------------------------------------------------------------------------------------------------------------------------
genvar i;

`ifdef   DEBUG
wire  [32:0]   imiso_c  [0:7];
`endif

wire [31:0] QPO; //JL

// Raceway Interconnect

wire  [383:0]   lane [0:8];
wire  [  3:0]   give [0:8];
wire  [  3:0]   take [0:8];
wire            rclk [0:8];

assign
   QI_ir   = take[0],
   lane[0] = QI,
   give[0] = QI_wr,
   rclk[0] = QI_ck,
   take[8] = QO_rd,     //***JL Replace ; with ,
   QO      = lane[8],
   QO_or   = give[8],   
   QO_ck   = rclk[8];   //***JL - Replace , with ;

// Generate Columns
for (i=0; i<8; i=i+1) begin : cols
   localparam ColID = {1'b0,i[2:0],1'b0};   // Columns 8..15 and rows
   if (i==0) begin
      ColumnZero QCZ (
         // Raceway Output
         .CO        (lane[i+1][383:0]),
         .CO_or     (give[i+1][  3:0]),
         .CO_rd     (take[i+1][  3:0]),
         .CO_ck     (rclk[i+1]),
         // Raceway Input
         .CI        (lane[ i ][383:0]),
         .CI_wr     (give[ i ][  3:0]),
         .CI_ir     (take[ i ][  3:0]),
         .CI_ck     (rclk[ i ]),
         // TileZero Interface
         // Output Interface
         .RT_zero   (RT_zero[97:0]),          // Raceway to TileZero xmosi
         .RT_zero_fb(RT_zero_fb ),
         .RT_zero_ck(RT_zero_ck ),
         // Input Interface
         .TR_zero   (TR_zero[97:0]),          // TileZero xmiso to Raceway
         .TR_zero_fb(TR_zero_fb ),
         .TR_zero_ck(TR_zero_ck ),
         // Global
`ifdef DEBUG
         .imiso     (imiso_c[i] ),           // Slave data out {                IOk, IOo[31:0]}
         .imosi     (imosi      ),           // Slave data in  {cIO[2:0], iIO[31:0], aIO[12:0]}
`endif
         .PUFosc    (QPO[4*i+:4]),
		 .TIMOo     (QTIMOo[8*i+:8]), //JL
         .FLAGo     (FLAGo[8*i+:8]),
         .FLAGi     (FLAGi[8*i+:8]),
		 .reset     (reset) //JL
         );
      end
   else begin
	//Column #(ColID) i_col ( //JLSW
	Column i_col ( //JLSW
         // Raceway Output
         .CO        (lane[i+1][383:0]),
         .CO_or     (give[i+1][  3:0]),
         .CO_rd     (take[i+1][  3:0]),
         .CO_ck     (rclk[i+1]),
         // Raceway Input
         .CI        (lane[ i ][383:0]),
         .CI_wr     (give[ i ][  3:0]),
         .CI_ir     (take[ i ][  3:0]),
         .CI_ck     (rclk[ i ]),
   `ifdef DEBUG
         .imiso     (imiso_c[i] ),           // Slave data out {                IOk, IOo[31:0]}
         .imosi     (imosi      ),           // Slave data in  {cIO[2:0], iIO[31:0], aIO[12:0]}
   `endif
         // Global
         .PUFosc    (QPO[4*i+:4]),
		 .TIMOo     (QTIMOo[8*i+:8]), //JL
         .FLAGo     (FLAGo[8*i+:8]),
         .FLAGi     (FLAGi[8*i+:8]),
         .reset     (reset)
         );
      end
   end

assign QPUFosc[3:0]  =  QPO[31:28] | QPO[27:24] | QPO[23:20] | QPO[19:16] | QPO[15:12] | QPO[11:8]  | QPO[7:4]   | QPO[3:0];

`ifdef   DEBUG
integer j;
always @* begin
   imiso = 33'h0;
   for (j=0;j<8;j=j+1)
      imiso = imiso | imiso_c[j];
   end
`endif

endmodule

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////


`ifdef   verify_quadrantzero

module t;

wire [383:0]  QO;
wire [  3:0]  QO_or;
reg  [  3:0]  QO_rd;
wire          QO_ck;
reg [383:0]   QI;
reg [  3:0]   QI_wr;
wire [  3:0]  QI_ir;
reg           QI_ck;
wire [ 97:0]  RT_zero;
reg           RT_zero_fb;
wire          RT_zero_ck;
reg  [ 97:0]  TR_zero;
wire          TR_zero_fb;
reg           TR_zero_ck;
wire [  3:0]  QPUFosc;
wire [ 63:0]  QTIMOo;
wire [ 63:0]  FLAGo;
reg  [ 63:0]  FLAGi;
reg           reset;


QuadrantZero  i_Q1 (
   .QO         (QO   ),
   .QO_or      (QO_or),
   .QO_rd      (QO_rd),
   .QO_ck      (QO_ck),
   .QI         (QI   ),
   .QI_wr      (QI_wr),
   .QI_ir      (QI_ir),
   .QI_ck      (QI_ck),
   .RT_zero    (RT_zero),
   .RT_zero_fb (RT_zero_fb),
   .RT_zero_ck (RT_zero_ck),
   .TR_zero    (TR_zero),
   .TR_zero_fb (TR_zero_fb),
   .TR_zero_ck (TR_zero_ck),
   .QPUFosc    (QPUFosc),
   .QTIMOo    (QTIMOo),
   .FLAGi     (FLAGi),
   .FLAGo     (FLAGo),
   .reset     (reset)
   );


initial begin
   QI_ck = 1'b0;
   forever
      QI_ck = #10 ~QI_ck;
   end

initial begin
   QO_rd    =    4'h0;
   QI       =  384'h0;
   QI_wr    =    4'h0;
   RT_zero_fb =  1'b0;
   TR_zero    = 98'h0;
   TR_zero_ck =  1'b0;
   FLAGi      = 64'h0;
   reset    =    1'b0;
   $display(" Reset");
   repeat (10)  @(posedge QI_ck); #1;
   QO_rd    =    4'h5;
   QI       =  384'h10;
   QI_wr    =    4'h8;
   RT_zero_fb =  1'b1;
   TR_zero    = 98'h20;
   TR_zero_ck =  1'b1;
   FLAGi      = 64'h30;
   reset    =    1'b1;

   repeat (1000) @(posedge QI_ck); #1;
   $display("\n All Tests Complete\n");
   $finish;
   $stop;
   end



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
