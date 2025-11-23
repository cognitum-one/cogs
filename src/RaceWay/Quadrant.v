//===============================================================================================================================
//
//    Copyright © 2022 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to Authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//    Project Name         : Newport ASIC
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
//      Four Quadrants 0,1,2 & 3   Quadrant[0] is a QuadrantZero and is upper left   [0ccc,0rrr]
//                                 Quadrant[1] is a QuadrantOne         upper right  [0ccc,1rrr]
//                                 Quadrant[2] is a QuadrantOne         lower left   [1ccc,0rrr]
//                                 Quadrant[3] is a QuadrantOne         lower right  [1ccc,1rrr]
//
//      Tile ID =  [cccc,rrrr]     column major   so  cccc = col  & rrrr = row
//
//      So quadrant ID [1:0]       msb is Row[3] and lsb is Column[3]   -- yes its backwards
//
//      QuadrantZero is the North-West Corner of the die and holds Tile Addresses [Columns 0..7  and Rows 0..7 ]  **1
//      Quadrant[1]  is the North-East Corner of the die and holds Tile Addresses [Columns 8..15 and Rows 0..7 ]
//      Quadrant[2]  is the South-Wset Corner of the die and holds Tile Addresses [Columns 0..7  and Rows 8..15]
//      Quadrant[3]  is the South-East Corner of the die and holds Tile Addresses [Columns 8..15 and Rows 8..15]
//
//       **1 QuadrantZero is a separate module with a Tile output to the top-level to connect to TileZero
//
//      Column ID needs Full column ID AND msb of Rows from the Quadrant ID.  So column ID is 5 bits; bit 4 is row msb and
//      bit 3 is the column (msb) and bits 2:0 are the generate index below
//
//      LOGICAL LAYOUT:
//
//         Column         Column         Column         Column         Column         Column         Column         Column
//            0              1              2              3              4              5              6              7
// QI      +-----+        +-----+        +-----+        +-----+        +-----+        +-----+        +-----+        +-----+    QO
// ==lane=>|di do|==lane=>|di do|==lane=>|di do|==lane=>|di do|==lane=>|di do|==lane=>|di do|==lane=>|di do|==lane=>|di do|=lane=>
//    0    |     |   1    |     |   2    |     |   3    |     |   4    |     |   5    |     |   6    |     |   7    |     |  8
// --give->|wr or|--give->|wr or|--give->|wr or|--give->|wr or|--give->|wr or|--give->|wr or|--give->|wr or|--give->|wr or|-give->
//         |     |        |     |        |     |        |     |        |     |        |     |        |     |        |     |
// <-take--|ir rd|<-take--|ir rd|<-take--|ir rd|<-take--|ir rd|<-take--|ir rd|<-take--|ir rd|<-take--|ir rd|<-take--|ir rd|<-take-
//         +-----+        +-----+        +-----+        +-----+        +-----+        +-----+        +-----+        +-----+
//
//      PHYSICAL LAYOUT:
//        Nota Bene:  Alternate Physical Blocks are rotated.
//        This is done so that to run from Column 7 is NOT across all 8 Columns & each lane length is only one column width.
//
//          Column      Column      Column      Column      Column      Column      Column      Column
//            4           3           5           2           6           1           7           0
//                  +-----------+           +-----------+           +-----------+           +-------------lane[8]--->> QO to Hub
//          +---+   |   +---+   |   +---+   |   +---+   |   +---+   |   +---+   |   +---+   |   +---+
//      +-->|\  |---+   |  /|   +-->|\  |---+   |  /|   +-->|\  |---+   |  /|   +-->|\  |---+   |  /|
//      |   | \ |       | / |       | \ |       | / |       | \ |       | / |       | \ |       | / |
//      |   |  \|   +---|/  |<--+   |  \|   +---|/  |<--+   |  \|   +---|/  |<--+   |  \|   +---|/  |<--+
//      |   +---+   |   +---+   |   +---+   |   +---+   |   +---+   |   +---+   |   +---+   |   +---+   |
//      +-----------+           +-----------+           +-----------+           +-----------+           +-lane[0]--<< QI from Hub
//
//===============================================================================================================================

module  Quadrant #(
   parameter   [  1:0]  Qid   =  1     // Must be 1, 2 or 3 : bit 1 is the ms Column ID and bit 0 is the ms Row ID
   )(
   // Output Interface
   output wire [383:0]  QO,
   output wire [  3:0]  QO_or,
   input  wire [  3:0]  QO_rd,
   output wire          QO_ck,
   // Input Interface
   input  wire [383:0]  QI,
   input  wire [  3:0]  QI_wr,
   output wire [  3:0]  QI_ir,
   input  wire          QI_ck,
`ifdef DEBUG
   input  wire [ 63:0]  go,            // Global start  for FakeGateway State Machine to initiate parallel operation
   output wire [ 63:0]  fini,          // Global finish for FakeGateway State Machine to end operation
   output wire [ 32:0]  imiso,         // Slave data out {                IOk, IOo[31:0]}
   input  wire [ 47:0]  imosi,         // Slave data in  {cIO[2:0], iIO[31:0], aIO[12:0]}
`endif
   // Global
   output wire [  3:0]  QPUFosc,
   output wire [ 63:0]  QTIMOo,
   output wire [ 63:0]  QFLAGo,
   input  wire [ 63:0]  QFLAGi,
   input  wire          reset
   );

// Nets ------------------------------------------------------------------------------------------------------------------------
genvar i;

wire [31:0] QPO; //JL

// Raceway Interconnect

wire  [383:0]   lane    [0:8];
wire  [  3:0]   give    [0:8];
wire  [  3:0]   take    [0:8];
wire            rclk    [0:8];
`ifdef DEBUG
wire  [ 32:0]   imiso_c [0:7];
`endif

assign
   QI_ir   = take[0],
   lane[0] = QI,
   give[0] = QI_wr,
   rclk[0] = QI_ck,
   take[8] = QO_rd,
   QO      = lane[8],
   QO_or   = give[8],
   QO_ck   = rclk[8];

// Generate Columns
for (i=0; i<8; i=i+1) begin : cols
   localparam ColID = {Qid[1],i[2:0],Qid[0]};   // Columns 8..15 and rows //JLPL - Swap temporary to match Hub config
`ifdef synthesis
   Column i_col (
`else 
   Column #(ColID) i_col (
`endif
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
      .fini      (fini[8*i+:8]),
      .imiso     (imiso_c[i]),
      .imosi     (imosi),                       // Slave data in  {cIO[2:0], iIO[31:0], aIO[12:0]}
      .go        (go[8*i+:8]),
`endif
      // Global
      .PUFosc    (QPO[4*i+:4]),
      .TIMOo     (QTIMOo[8*i+:8]),
      .FLAGo     (QFLAGo[8*i+:8]),
      .FLAGi     (QFLAGi[8*i+:8]),
      .reset     (reset)
      );
   end

assign QPUFosc[3:0]  =  QPO[31:28] | QPO[27:24] | QPO[23:20] | QPO[19:16] | QPO[15:12] | QPO[11:8]  | QPO[7:4]   | QPO[3:0];

`ifdef DEBUG
assign imiso = imiso_c[0] | imiso_c[1] | imiso_c[2] | imiso_c[3] | imiso_c[4] | imiso_c[5] | imiso_c[6] | imiso_c[7];
`endif

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
`ifdef   verify_quadrant

module t;

integer  i,j,k, count, testnum;
// Quadrant One location 1  deals with Tiles  8..15 and rows 0..7        Tile #s for Quadrant 1    Index values
//                                                                       80 90 a0 b0 c0 d0 e0 f0   00 08 16 24 32 40 48 56
//                                                                       81 91 a1 b1 c1 d1 e1 f1   01 09 17 25 33 41 49 57
//                                                                       82 92 a2 b2 c2 d2 e2 f2   02 10 18 26 34 42 50 58
//                                                                       83 93 a3 b3 c3 d3 e3 f3   03 11 19 27 35 43 51 59
//                                                                       84 94 a4 b4 c4 d4 e4 f4   04 12 20 28 36 44 52 60
//                                                                       85 95 a5 b5 c5 d5 e5 f5   05 13 21 29 37 45 53 61
//                                                                       86 96 a6 b6 c6 d6 e6 f6   06 14 22 30 38 46 54 62
//                                                                       87 97 a7 b7 c7 d7 e7 f7   07 15 23 31 39 47 55 63

//initial begin
//   for (i=0;i<64;i=i+1) begin
//      j = 8'h80 + (i%8) + (i/8)*8'h10;
//      $display(" i=%h   => %h",i,j);
//      end
//   end

wire [383:0]  QO;
wire [  3:0]  QO_or;
wire [  3:0]  QO_rd;
wire          QO_ck;
wire [383:0]  QI;
wire [  3:0]  QI_wr;
wire [  3:0]  QI_ir;
reg           QI_ck;
wire [ 63:0]  QTIMOo;
wire [ 63:0]  QFLAGo;
reg  [ 63:0]  QFLAGi;
reg           reset;
wire [ 63:0]  fini;
reg  [ 63:0]  go;
wire [ 32:0]  imiso;         // Slave data out {                IOk, IOo[31:0]}
reg  [ 47:0]  imosi;         // Slave data in  {cIO[2:0], iIO[31:0], aIO[12:0]}
reg           all;           // Enable fake imosi to write to all Fake Tiles at once

Quadrant #(1) i_Q1 (
   .QO      (QO   ),
   .QO_or   (QO_or),
   .QO_rd   (QO_rd),
   .QO_ck   (QO_ck),
   .QI      (QI   ),
   .QI_wr   (QI_wr),
   .QI_ir   (QI_ir),
   .QI_ck   (QI_ck),
   .QFLAGi  (QFLAGi),
   .QFLAGo  (QFLAGo),
   .QTIMOo  (QTIMOo),
`ifdef DEBUG
   .go      (go   ),
   .fini    (fini),
   .imiso   (imiso),         // Slave data out {                IOk, IOo[31:0]}
   .imosi   (imosi),         // Slave data in  {cIO[2:0], iIO[31:0], aIO[12:0]}
`endif
   .reset   (reset)
   );

// Loopback QO to QI for testing
assign
   QO_rd    =  QI_ir,
   QI_wr    =  QO_or,
   QI       =  QO;

reg   clock;
initial begin
   clock = 1'b0;
   forever
      clock = #10 ~clock;
   end

   //integer i;
initial begin
   for (i=0; i<96; i=i+1) $write(" %2d",95-i);
   $display(" ");
   for (i=0; i<96; i=i+1) $write(" %2d",95-((i%8)*12+(i/8)));
   $display(" ");
   end

always @* QI_ck = clock;

reg         good1,good2,good3,good;
reg   [7:0] src,dst,idx;
reg   [7:0] src1, idx1, dst1, src2, idx2, dst2;
reg         bcast1,  bcast2;
reg   [7:0] source;
reg   [7:0] destin;
reg   [7:0] temp;
reg   [5:0] tmp2;
reg  [63:0] os, nos;
reg         load_os;

//always @* for (i=0;i<9;i=i+1) begin
//   lane_src_0[i]  = i_Q1.lane[i][071:064];
//   lane_src_1[i]  = i_Q1.lane[i][167:160];
//   lane_src_2[i]  = i_Q1.lane[i][263:256];
//   lane_src_3[i]  = i_Q1.lane[i][359:352];
//   end

reg  [31:0] mem [0:63];
always @* begin
   mem[00] = i_Q1.cols[0].i_col.Tiles[0].iT01.RAM[10];
   mem[01] = i_Q1.cols[0].i_col.Tiles[1].iT01.RAM[10];
   mem[02] = i_Q1.cols[0].i_col.Tiles[2].iT01.RAM[10];
   mem[03] = i_Q1.cols[0].i_col.Tiles[3].iT01.RAM[10];
   mem[04] = i_Q1.cols[0].i_col.Tiles[4].iT01.RAM[10];
   mem[05] = i_Q1.cols[0].i_col.Tiles[5].iT01.RAM[10];
   mem[06] = i_Q1.cols[0].i_col.Tiles[6].iT01.RAM[10];
   mem[07] = i_Q1.cols[0].i_col.Tiles[7].iT01.RAM[10];
   mem[08] = i_Q1.cols[1].i_col.Tiles[0].iT01.RAM[10];
   mem[09] = i_Q1.cols[1].i_col.Tiles[1].iT01.RAM[10];
   mem[10] = i_Q1.cols[1].i_col.Tiles[2].iT01.RAM[10];
   mem[11] = i_Q1.cols[1].i_col.Tiles[3].iT01.RAM[10];
   mem[12] = i_Q1.cols[1].i_col.Tiles[4].iT01.RAM[10];
   mem[13] = i_Q1.cols[1].i_col.Tiles[5].iT01.RAM[10];
   mem[14] = i_Q1.cols[1].i_col.Tiles[6].iT01.RAM[10];
   mem[15] = i_Q1.cols[1].i_col.Tiles[7].iT01.RAM[10];
   mem[16] = i_Q1.cols[2].i_col.Tiles[0].iT01.RAM[10];
   mem[17] = i_Q1.cols[2].i_col.Tiles[1].iT01.RAM[10];
   mem[18] = i_Q1.cols[2].i_col.Tiles[2].iT01.RAM[10];
   mem[19] = i_Q1.cols[2].i_col.Tiles[3].iT01.RAM[10];
   mem[20] = i_Q1.cols[2].i_col.Tiles[4].iT01.RAM[10];
   mem[21] = i_Q1.cols[2].i_col.Tiles[5].iT01.RAM[10];
   mem[22] = i_Q1.cols[2].i_col.Tiles[6].iT01.RAM[10];
   mem[23] = i_Q1.cols[2].i_col.Tiles[7].iT01.RAM[10];
   mem[24] = i_Q1.cols[3].i_col.Tiles[0].iT01.RAM[10];
   mem[25] = i_Q1.cols[3].i_col.Tiles[1].iT01.RAM[10];
   mem[26] = i_Q1.cols[3].i_col.Tiles[2].iT01.RAM[10];
   mem[27] = i_Q1.cols[3].i_col.Tiles[3].iT01.RAM[10];
   mem[28] = i_Q1.cols[3].i_col.Tiles[4].iT01.RAM[10];
   mem[29] = i_Q1.cols[3].i_col.Tiles[5].iT01.RAM[10];
   mem[30] = i_Q1.cols[3].i_col.Tiles[6].iT01.RAM[10];
   mem[31] = i_Q1.cols[3].i_col.Tiles[7].iT01.RAM[10];
   mem[32] = i_Q1.cols[4].i_col.Tiles[0].iT01.RAM[10];
   mem[33] = i_Q1.cols[4].i_col.Tiles[1].iT01.RAM[10];
   mem[34] = i_Q1.cols[4].i_col.Tiles[2].iT01.RAM[10];
   mem[35] = i_Q1.cols[4].i_col.Tiles[3].iT01.RAM[10];
   mem[36] = i_Q1.cols[4].i_col.Tiles[4].iT01.RAM[10];
   mem[37] = i_Q1.cols[4].i_col.Tiles[5].iT01.RAM[10];
   mem[38] = i_Q1.cols[4].i_col.Tiles[6].iT01.RAM[10];
   mem[39] = i_Q1.cols[4].i_col.Tiles[7].iT01.RAM[10];
   mem[40] = i_Q1.cols[5].i_col.Tiles[0].iT01.RAM[10];
   mem[41] = i_Q1.cols[5].i_col.Tiles[1].iT01.RAM[10];
   mem[42] = i_Q1.cols[5].i_col.Tiles[2].iT01.RAM[10];
   mem[43] = i_Q1.cols[5].i_col.Tiles[3].iT01.RAM[10];
   mem[44] = i_Q1.cols[5].i_col.Tiles[4].iT01.RAM[10];
   mem[45] = i_Q1.cols[5].i_col.Tiles[5].iT01.RAM[10];
   mem[46] = i_Q1.cols[5].i_col.Tiles[6].iT01.RAM[10];
   mem[47] = i_Q1.cols[5].i_col.Tiles[7].iT01.RAM[10];
   mem[48] = i_Q1.cols[6].i_col.Tiles[0].iT01.RAM[10];
   mem[49] = i_Q1.cols[6].i_col.Tiles[1].iT01.RAM[10];
   mem[50] = i_Q1.cols[6].i_col.Tiles[2].iT01.RAM[10];
   mem[51] = i_Q1.cols[6].i_col.Tiles[3].iT01.RAM[10];
   mem[52] = i_Q1.cols[6].i_col.Tiles[4].iT01.RAM[10];
   mem[53] = i_Q1.cols[6].i_col.Tiles[5].iT01.RAM[10];
   mem[54] = i_Q1.cols[6].i_col.Tiles[6].iT01.RAM[10];
   mem[55] = i_Q1.cols[6].i_col.Tiles[7].iT01.RAM[10];
   mem[56] = i_Q1.cols[7].i_col.Tiles[0].iT01.RAM[10];
   mem[57] = i_Q1.cols[7].i_col.Tiles[1].iT01.RAM[10];
   mem[58] = i_Q1.cols[7].i_col.Tiles[2].iT01.RAM[10];
   mem[59] = i_Q1.cols[7].i_col.Tiles[3].iT01.RAM[10];
   mem[60] = i_Q1.cols[7].i_col.Tiles[4].iT01.RAM[10];
   mem[61] = i_Q1.cols[7].i_col.Tiles[5].iT01.RAM[10];
   mem[62] = i_Q1.cols[7].i_col.Tiles[6].iT01.RAM[10];
   mem[63] = i_Q1.cols[7].i_col.Tiles[7].iT01.RAM[10];
   end

initial begin
   load_os  =       0;
   i        =       0;
   all      =    1'b0;
   good     =    1'b0;
   QFLAGi   =   64'h0;
   go       =   64'h0;
   reset    =    1'b0;
   imosi    =   48'h0;
   $display(" Reset");
   repeat (10)  @(posedge clock); #1;
   reset    =    1'b1;
   repeat (10)  @(posedge clock); #1;
   reset    =    1'b0;
   $display(" Initial Check");
   repeat (10)  @(posedge clock); #1;
   imosi    =  {3'b101,32'h00000000,13'h0800};     // Read  myID     1'b1, Column 8 Row 0 (8'h80), GW reg address[3:0])
   repeat (01)  @(posedge clock); #1;
   imosi    =  {3'b110,32'h00000000,13'h0801};     // Write ADDR
   repeat (01)  @(posedge clock); #1;
   imosi    =  {3'b110,32'h600dbabe,13'h0802};     // Write DATA
   repeat (01)  @(posedge clock); #1;
   imosi    =  {3'b110,32'h91008080,13'h0803};     // Write CMND
   go[00]   =   1'b1;
   repeat (01)  @(posedge clock); #1;
   go[00]   =   1'b0;
   imosi    =  {3'b0xx,32'hdeaddead,13'h0000};
   repeat (01)  @(posedge clock); #1;
// $display(" fini[%2d] = %b initial",0,fini[0]);
   while (fini[0] !== 1'b1) begin
      @(posedge clock); #1;
      end
   repeat (01)  @(posedge clock); #1;
   // Check results
   imosi    =  {3'b101,32'h00000000,13'h0805};
   @(posedge clock); #10;
   good     =           imiso[32:0] === 33'h1600dfeed;
   imosi    =  {3'b101,32'h00000000,13'h0806};
   @(posedge clock); #10;
   good     =   good & (imiso[32:0] === 33'h1600dbabe);
   imosi    =  {3'b101,32'h00000000,13'h0807};
   @(posedge clock); #10;
   good     =   good & (imiso[32:0] === 33'h111008080);
   if (!good)  $display("Error respnse with %h to %h",imiso[7:0],imiso[15:8]);
   repeat (01)  @(posedge clock); #1;
   good     =  1'b0;
   i=0;

   testnum = 1;
   $display("\n Test %2d, Test all Tiles to same Self\n", testnum);
   for (i=0;i<64;i=i+1) begin
      k = 8'h80 + (i%8) + (i/8)*8'h10;
      src[7:0] = k;
      dst[7:0] = k;
      idx[7:0] = i;
      point2point (all,32'h00000004,{16'hbabe,src,dst},{8'h91,8'h00,dst,src});
      end

   repeat (50)  @(posedge clock); #1;

   testnum = 2;
   $display("\n Test %2d, Test all Tiles to other Tiles in the same column\n", testnum);
   for (i=0;i<64;i=i+1) begin
      k = 8'h80 + (i%8) + (i/8)*8'h10;
      src[7:0] = k;
      for(j=0;j<8;j=j+1) begin
         dst[7:0] = src + j - (i%8);
         idx[7:0] = i;
         point2point (all,32'h00000004,{16'hbabe,src,dst},{8'h91,8'h00,dst,src});
         end
      end

   testnum = 3;
   $display("\n Test %2d, Test Tiles to Tiles in different column\n", testnum);

   // idx is the index to the correct gateway to check for state machine finished. There 64 per quadrant as in top comment in TB
   // and, yes, easiest to see in octal!!                    address       data               cmd   tag   dst src
   src = 8'h80; dst = 8'h97; idx = 8'o000;  point2point (all,32'h00000008,{16'hbabe,src,dst},{8'h91,8'h00,dst,src});
   src = 8'h91; dst = 8'ha6; idx = 8'o011;  point2point (all,32'h00000008,{16'hbabe,src,dst},{8'h91,8'h00,dst,src});
   src = 8'ha2; dst = 8'hb5; idx = 8'o022;  point2point (all,32'h00000008,{16'hbabe,src,dst},{8'h91,8'h00,dst,src});
   src = 8'hb3; dst = 8'hc4; idx = 8'o033;  point2point (all,32'h00000008,{16'hbabe,src,dst},{8'h91,8'h00,dst,src});
   src = 8'hc4; dst = 8'hd3; idx = 8'o044;  point2point (all,32'h00000008,{16'hbabe,src,dst},{8'h91,8'h00,dst,src});
   src = 8'hd5; dst = 8'he2; idx = 8'o055;  point2point (all,32'h00000008,{16'hbabe,src,dst},{8'h91,8'h00,dst,src});
   src = 8'he6; dst = 8'hf1; idx = 8'o066;  point2point (all,32'h00000008,{16'hbabe,src,dst},{8'h91,8'h00,dst,src});
   src = 8'hf7; dst = 8'h80; idx = 8'o077;  point2point (all,32'h00000008,{16'hbabe,src,dst},{8'h91,8'h00,dst,src});

   testnum = 4;
   $display("\n Test %2d, Test Tile Broadcast to all Tiles in the Quadrant\n", testnum);
   src = 8'h85; dst = 8'h00; idx = 8'o005;  point2point (all,32'h0000000c,{16'hbabe,src,dst},{8'hb1,8'h00,dst,src});
   src = 8'h93; dst = 8'h00; idx = 8'o013;  point2point (all,32'h00000010,{16'hbeef,src,dst},{8'hb1,8'h00,dst,src});
   src = 8'ha6; dst = 8'h00; idx = 8'o026;  point2point (all,32'h00000014,{16'hf1d0,src,dst},{8'hb1,8'h00,dst,src});
   src = 8'hb0; dst = 8'h00; idx = 8'o030;  point2point (all,32'h00000018,{16'hfade,src,dst},{8'hb1,8'h00,dst,src});
   src = 8'hc1; dst = 8'h00; idx = 8'o041;  point2point (all,32'h0000001c,{16'hfeed,src,dst},{8'hb1,8'h00,dst,src});
   src = 8'hd7; dst = 8'h00; idx = 8'o057;  point2point (all,32'h00000020,{16'hfade,src,dst},{8'hb1,8'h00,dst,src});
   src = 8'he2; dst = 8'h00; idx = 8'o062;  point2point (all,32'h00000024,{16'hab1e,src,dst},{8'hb1,8'h00,dst,src});
   src = 8'hf4; dst = 8'h00; idx = 8'o074;  point2point (all,32'h00000028,{16'hdeed,src,dst},{8'hb1,8'h00,dst,src});

   repeat (10) @(posedge clock); #1;

   $display(" ");
   for (i=0;i<64;i=i+1)
      if (i==0)                        $display(" Tile[%2d] received %h",i,mem[i]);
      else if (mem[i] !== mem[i-1])    $display(" Tile[%2d] received %h -- all the same until \n Tile[%2d] received %h -- value changed ",i-1,mem[i-1],i,mem[i]);

   testnum = 5;
   $display("\n Test %2d, Test Two accesses at the same time within a Tile\n", testnum);
   src1 = 8'h80; dst1 = 8'h85; idx1 = 8'o000;  //        address       data                 command
   src2 = 8'h84; dst2 = 8'h80; idx2 = 8'o004;  twoByTwo (32'h00000030,{16'hcaca,src1,dst1},{8'h91,8'h00,dst1,src1},
                                                         32'h00000030,{16'hdada,src2,dst2},{8'h91,8'h00,dst2,src2});
   src1 = 8'h80; dst1 = 8'h85; idx1 = 8'o000;
   src2 = 8'h84; dst2 = 8'h80; idx2 = 8'o004;  twoByTwo (32'h00000034,{16'hbeef,src1,dst1},{8'h91,8'h00,dst1,src1},
                                                         32'h00000034,{16'hbabe,src2,dst2},{8'h91,8'h00,dst2,src2});
   src1 = 8'h80; dst1 = 8'h85; idx1 = 8'o000;
   src2 = 8'h84; dst2 = 8'h80; idx2 = 8'o004;  twoByTwo (32'h00000030,{32'h0             },{8'h89,8'h00,dst1,src1},
                                                         32'h00000030,{32'h0             },{8'h89,8'h00,dst2,src2});
   testnum = 6;
   $display("\n Test %2d, Test Two accesses at the same time to a single Tile\n", testnum);
   src1 = 8'h80; dst1 = 8'h85; idx1 = 8'o000;  //        address       data                 command
   src2 = 8'h84; dst2 = 8'h85; idx2 = 8'o004;  twoByTwo (32'h00000040,{16'hface,src1,dst1},{8'h91,8'h00,dst1,src1},
                                                         32'h00000044,{16'hcafe,src2,dst2},{8'h91,8'h00,dst2,src2});
   src1 = 8'h80; dst1 = 8'h85; idx1 = 8'o000;  //        address       data                 command
   src2 = 8'h84; dst2 = 8'h85; idx2 = 8'o004;  twoByTwo (32'h00000040,{32'h00000000},      {8'h89,8'h00,dst1,src1},
                                                         32'h00000044,{32'h00000000},      {8'h89,8'h00,dst2,src2});
   testnum = 7;
   $display("\n Test %2d, Test Two accesses at the same time to a single Tile in a different Column \n", testnum);
   src1 = 8'hb1; dst1 = 8'h85; idx1 = 8'o031;  //        address       data                 command
   src2 = 8'hb5; dst2 = 8'h85; idx2 = 8'o035;  twoByTwo (32'h00000040,{16'hdace,src1,dst1},{8'h91,8'h00,dst1,src1},
                                                         32'h00000044,{16'hcade,src2,dst2},{8'h91,8'h00,dst2,src2});
   src1 = 8'hb1; dst1 = 8'h85; idx1 = 8'o031;  //        address       data                 command
   src2 = 8'hb5; dst2 = 8'h85; idx2 = 8'o035;  twoByTwo (32'h00000040,{32'h00000000},      {8'h89,8'h00,dst1,src1},
                                                         32'h00000044,{32'h00000000},      {8'h89,8'h00,dst2,src2});
   testnum = 8;
   $display("\n Test %2d, Crazy test!  All Tiles to one Tile \n", testnum);

   for (i=0;i<64;i=i+1) begin
      source   =  {1'b1, i[5:3], 1'b0, i[2:0]};
      imosi    =  {3'b110, {24'h0,i[5:0],2'b0},  {1'b0,source,4'h1}};  @(posedge clock); #1;  // RAM address
      imosi    =  {3'b110, {24'habba80,source},  {1'b0,source,4'h2}};  @(posedge clock); #1;  // RAM data in
      imosi    =  {3'b110, {24'h910080,source},  {1'b0,source,4'h3}};  @(posedge clock); #1;  // Command
      imosi    =  {3'b110, {32'h00000000     },  {1'b0,source,4'h5}};  @(posedge clock); #1;  // Clear
      imosi    =  {3'b110, {32'h00000000     },  {1'b0,source,4'h6}};  @(posedge clock); #1;  // Clear
      imosi    =  {3'b110, {32'h00000000     },  {1'b0,source,4'h7}};  @(posedge clock); #1;  // Clear
      @(posedge clock); #1;
      end
   go = 64'hffffffffffffffff;  @(posedge clock); #1;
   go = 64'h0000000000000000;  @(posedge clock); #1;
   os = 64'h0000000000000000;
   repeat (2)  @(posedge clock); #1;

   count = 0;
   repeat (144) begin
      count = count +1;
      for (i=0;i<9;i=i+1) begin $write(" %h  ",i_Q1.lane[(i+1)%9][071:064]); end $display("  => race[] = %h %h", i_Q1.cols[1].i_col.iRW1.A[3].iRP.Skid[71:64], i_Q1.cols[0].i_col.iRW1.A[3].iRP.Pipe[71:64]);
      for (i=0;i<9;i=i+1) begin $write(" %h  ",i_Q1.lane[(i+1)%9][167:160]); end $display("  => race[] = %h %h", i_Q1.cols[1].i_col.iRW1.A[2].iRP.Skid[71:64], i_Q1.cols[0].i_col.iRW1.A[2].iRP.Pipe[71:64]);
      for (i=0;i<9;i=i+1) begin $write(" %h  ",i_Q1.lane[(i+1)%9][263:256]); end $display("  => race[] = %h %h", i_Q1.cols[1].i_col.iRW1.A[1].iRP.Skid[71:64], i_Q1.cols[0].i_col.iRW1.A[1].iRP.Pipe[71:64]);
      for (i=0;i<9;i=i+1) begin $write(" %h  ",i_Q1.lane[(i+1)%9][359:352]); end $display("  => race[] = %h %h", i_Q1.cols[1].i_col.iRW1.A[0].iRP.Skid[71:64], i_Q1.cols[0].i_col.iRW1.A[0].iRP.Pipe[71:64]);
      temp[7:0] = i_Q1.cols[0].i_col.Tiles[0].iT01.from;
      tmp2[5:0] = {temp[6:4],temp[2:0]};
      load_os   = 1;
      nos       = os | (1 << tmp2);
      $display("%2d                                       ====>  %h    %d => %b", count, temp[7:0], tmp2, nos[63:0]);
      @(posedge clock); #1;
      os        = nos;
      end
   load_os = 0;

   testnum = 9;
   $display("\n Test %2d, Crazy test!  Almost All Tiles to one Tile \n", testnum);

   for (i=0;i<64;i=i+1) begin
      source   =  {1'b1, i[5:3], 1'b0, i[2:0]};
      imosi    =  {3'b110, {24'h0,i[5:0],2'b0},  {1'b0,source,4'h1}};  @(posedge clock); #1;  // RAM address
      imosi    =  {3'b110, {24'hbaab80,source},  {1'b0,source,4'h2}};  @(posedge clock); #1;  // RAM data in
      imosi    =  {3'b110, {24'h910080,source},  {1'b0,source,4'h3}};  @(posedge clock); #1;  // Command
      imosi    =  {3'b110, {32'h00000000     },  {1'b0,source,4'h5}};  @(posedge clock); #1;  // Clear
      imosi    =  {3'b110, {32'h00000000     },  {1'b0,source,4'h6}};  @(posedge clock); #1;  // Clear
      imosi    =  {3'b110, {32'h00000000     },  {1'b0,source,4'h7}};  @(posedge clock); #1;  // Clear
      @(posedge clock); #1;
      end
   go = 64'hffffffffffffffff;  @(posedge clock); #1;
   go = 64'h0000000000000000;  @(posedge clock); #1;
   os = 64'h0000000000000000;

   repeat (2)  @(posedge clock); #1;
   count = 0;
   repeat (144) begin
      count = count +1;
      for (i=0;i<9;i=i+1) begin $write(" %h  ",i_Q1.lane[i][071:064]); end $display("  => race[] = %h", i_Q1.cols[0].i_col.iRW1.rw[3][71:64]);
      for (i=0;i<9;i=i+1) begin $write(" %h  ",i_Q1.lane[i][167:160]); end $display("  => race[] = %h", i_Q1.cols[0].i_col.iRW1.rw[2][71:64]);
      for (i=0;i<9;i=i+1) begin $write(" %h  ",i_Q1.lane[i][263:256]); end $display("  => race[] = %h", i_Q1.cols[0].i_col.iRW1.rw[1][71:64]);
      for (i=0;i<9;i=i+1) begin $write(" %h  ",i_Q1.lane[i][359:352]); end $display("  => race[] = %h", i_Q1.cols[0].i_col.iRW1.rw[0][71:64]);
      temp[7:0] = i_Q1.cols[0].i_col.iRW1.RT[71:64];
      tmp2[5:0] = {temp[6:4],temp[2:0]};
      load_os   = 1;
      nos       = os | (1 << tmp2);
      $display("%2d                                       ====>  %h    %d => %b",count, temp[7:0], tmp2, nos[63:0]);
      @(posedge clock); #1;
      os        = nos;
      end
   load_os = 0;

   testnum = 10;
   $display("\n Test %2d, Joel's test case  \n", testnum);

   go = 64'h0000000000000000;  @(posedge clock); #1;

   src = 8'hb0; dst = 8'h80; idx = 8'o030;  point2point (all,32'h00000b00,{16'hfade,src,dst},{8'h91,8'h00,dst,src});
   src = 8'hb1; dst = 8'h00; idx = 8'o031;  point2point (all,32'h00000b10,{16'hbabe,src,dst},{8'hb1,8'h00,dst,src});
   src = 8'hb2; dst = 8'h80; idx = 8'o032;  point2point (all,32'h00000b20,{16'hfade,src,dst},{8'h91,8'h00,dst,src});
   src = 8'hb3; dst = 8'h80; idx = 8'o033;  point2point (all,32'h00000b30,{16'hfade,src,dst},{8'h91,8'h00,dst,src});
   src = 8'hb4; dst = 8'h80; idx = 8'o034;  point2point (all,32'h00000b40,{16'hfade,src,dst},{8'h91,8'h00,dst,src});

   @(posedge clock); #1;
   go = 64'h00_00_00_00_1f_00_00_00;  @(posedge clock); #1;
   go = 64'h0000000000000000;  @(posedge clock); #1;
   os = 64'h0000000000000000;

   repeat (2)  @(posedge clock); #1;
   count = 0;
   repeat (16) begin
      count = count +1;
      for (i=0;i<9;i=i+1) begin $write(" %h  ",i_Q1.lane[i][071:064]); end $display("  => race[] = %h", i_Q1.cols[0].i_col.iRW1.rw[3][71:64]);
      for (i=0;i<9;i=i+1) begin $write(" %h  ",i_Q1.lane[i][167:160]); end $display("  => race[] = %h", i_Q1.cols[0].i_col.iRW1.rw[2][71:64]);
      for (i=0;i<9;i=i+1) begin $write(" %h  ",i_Q1.lane[i][263:256]); end $display("  => race[] = %h", i_Q1.cols[0].i_col.iRW1.rw[1][71:64]);
      for (i=0;i<9;i=i+1) begin $write(" %h  ",i_Q1.lane[i][359:352]); end $display("  => race[] = %h", i_Q1.cols[0].i_col.iRW1.rw[0][71:64]);
      temp[7:0] = i_Q1.cols[0].i_col.iRW1.RT[71:64];
      tmp2[5:0] = {temp[6:4],temp[2:0]};
      load_os   = 1;
      nos       = os | (1 << tmp2);
      $display("%2d                                       ====>  %h    %d => %b",count, temp[7:0], tmp2, nos[63:0]);
      @(posedge clock); #1;
      os        = nos;
      end
   load_os = 0;

   repeat (1000) @(posedge clock); #1;
   $display("\n All %2d Tests Complete\n", testnum);
   $finish;
   $stop;
   end

//===============================================================================================================================


task  point2point;
input        all;
input [31:0] address;
input [31:0] data;
input [31:0] command;
reg   [ 7:0] source, destination;
reg          broadcast;
begin
   source      =   command[ 7:0];
   destination =   command[15:8];
   broadcast   =   command[29];
   $display(" point-to-point test %2d from source %h to destination %h",idx,source[7:0],destination[7:0]);
   imosi    =  {3'b110,  address,      {1'b0,source[7:0],4'h1}};  @(posedge clock); #1;
   imosi    =  {3'b110,  data,         {1'b0,source[7:0],4'h2}};  @(posedge clock); #1;
   imosi    =  {3'b110,  command,      {1'b0,source[7:0],4'h3}};  @(posedge clock); #1;
   go[idx]  =   1'b1;
   imosi    =  {3'b0xx,32'hdeaddead,13'h0000};                    @(posedge clock); #1;
   imosi    =  {48'h0};
   go[idx]  =   1'b0;
   while (fini[idx] !== 1'b1) begin
      @(posedge clock); #1;
      end
   repeat (01)  @(posedge clock); #1;
   imosi    =  {3'b101,32'h00000000,   {1'b0,source[7:0],4'h5}};  @(posedge clock); #10;
   good1    =   broadcast | (imiso[32:0] === 33'h1600dfeed);

   imosi    =  {3'b101,32'h00000000,   {1'b0,source[7:0],4'h6}};  @(posedge clock); #10;
   good2    =   broadcast | (imiso[32:0] === {1'b1,data});

   imosi    =  {3'b101,32'h00000000,   {1'b0,source[7:0],4'h7}};  @(posedge clock); #10;
   good3    =   broadcast | (imiso[32:0] === {17'h11100,source[7:0],destination[7:0]});

   if (!good3 && !broadcast)
                $display(" Error response good3 error found %h, expected %h ",imiso[32:0],{17'h11100,source,destination});
   good     =   good1 & good2 & good3;
   if (!good)   $display(" Error response with %h to %h, good= %b%b%b",source,destination,good3,good2,good1);
   repeat (01)  @(posedge clock); #1;
   good     = 0; good1 = 0; good2 = 0; good3 = 0;
   // Clear it out so easier to see on waveform
   imosi    =  {3'b110,  32'h0, {1'b0,source[7:0],4'h5}}; @(posedge clock); #1;
   imosi    =  {3'b110,  32'h0, {1'b0,source[7:0],4'h6}}; @(posedge clock); #1;
   imosi    =  {3'b110,  32'h0, {1'b0,source[7:0],4'h7}}; @(posedge clock); #1;
   imosi    =  {48'h0};
   end
endtask

task  twoByTwo;
input [31:0] addr1;
input [31:0] data1;
input [31:0] cmnd1;
input [31:0] addr2;
input [31:0] data2;
input [31:0] cmnd2;
reg   [ 7:0] source1,  source2,  destin1, destin2, index1, index2;
reg          bcast1,bcast2, read1, read2;
begin
   source1  =   cmnd1[ 7:0];  index1 = {2'b00,cmnd1[6:4],cmnd1[2:0]};
   destin1  =   cmnd1[15:8];
   source2  =   cmnd2[ 7:0];  index2 = {2'b00,cmnd2[6:4],cmnd2[2:0]};
   destin2  =   cmnd2[15:8];
   bcast1   =   cmnd1[29];
   bcast2   =   cmnd2[29];
   read1    =   cmnd1[92:91] == 2'b01;
   read2    =   cmnd2[92:91] == 2'b01;
   $display(" 2-by-2 test %2d  from source %h to destination %h and from source %h to destination %h",idx1,src1,dst1,src2,dst2);
   imosi    =  {3'b110,  addr1,  {1'b0,source1[7:0],4'h1}};  @(posedge clock); #1;
   imosi    =  {3'b110,  data1,  {1'b0,source1[7:0],4'h2}};  @(posedge clock); #1;
   imosi    =  {3'b110,  cmnd1,  {1'b0,source1[7:0],4'h3}};  @(posedge clock); #1;
   imosi    =  {3'b110,  addr2,  {1'b0,source2[7:0],4'h1}};  @(posedge clock); #1;
   imosi    =  {3'b110,  data2,  {1'b0,source2[7:0],4'h2}};  @(posedge clock); #1;
   imosi    =  {3'b110,  cmnd2,  {1'b0,source2[7:0],4'h3}};  @(posedge clock); #1;
   go[idx1] =   1'b1;
   go[idx2] =   1'b1;
   imosi    =   48'h0;                                       @(posedge clock); #1;
   go[idx1] =   1'b0;
   go[idx2] =   1'b0;
   while ((fini[index1] !== 1'b1) && (fini[index2] !== 1'b1)) begin
      @(posedge clock); #1;
      end
// $display(" Finito Bonito Jo");
   repeat (01)  @(posedge clock); #1;
   imosi    =  {3'b101, 32'h00000000, {1'b0,source1[7:0],4'h5}};  @(posedge clock); #10;     // Read Data 1
   good1    =   bcast2 | (read1 ? (imiso[32:0] === 33'h13ead600d) : (imiso[32:0] === 33'h1600dfeed));
   imosi    =  {3'b101,32'h00000000,  {1'b0,source1[7:0],4'h6}};  @(posedge clock); #10;     // Read Data 0
   good2    =   bcast2 | (read1 ? 1'b1 : (imiso[32:0] === {1'b1,data1}));
   imosi    =  {3'b101,32'h00000000,  {1'b0,source1[7:0],4'h7}};  @(posedge clock); #10;     // Response
   good3    =   bcast1 | (read1 ? (imiso[32:0] === {17'h10900,src1[7:0],dst1[7:0]})
                                : (imiso[32:0] === {17'h11100,src1[7:0],dst1[7:0]}));
   if (!good3 && !bcast1)
                $display(" Error response source1 error found %h, expected %h ",imiso[32:0],{17'h11100,source1,destin1});
   good     =   good1 & good2 & good3;
   if (!good)   $display(" Error response source1 with %h to %h, good= %b%b%b",source1,destin1,good3,good2,good1);
   repeat (01)  @(posedge clock); #1;
   good     = 0; good1 = 0; good2 = 0; good3 = 0;
   imosi    =  {3'b101, 32'h00000000, {1'b0,source2[7:0],4'h5}};  @(posedge clock); #10;     // Read Data 1
   good1    =   bcast2 | (read2 ? (imiso[32:0] === 33'h13ead600d) : (imiso[32:0] === 33'h1600dfeed));
   imosi    =  {3'b101,32'h00000000,  {1'b0,source2[7:0],4'h6}};  @(posedge clock); #10;     // Read Data 0
   good2    =   bcast2 | (read2 ? 1'b1 : (imiso[32:0] === {1'b1,data2}));
   imosi    =  {3'b101,32'h00000000,  {1'b0,source2[7:0],4'h7}};  @(posedge clock); #10;     // Response
   good3    =   bcast1 | (read2 ? (imiso[32:0] === {17'h10900,src2[7:0],dst2[7:0]})
                                : (imiso[32:0] === {17'h11100,src2[7:0],dst2[7:0]}));
   if (!good3 && !bcast1)
                $display(" Error response source2 good3 error found %h, expected %h ",imiso[32:0],{17'h11100,source2,destin2});
   good     =   good1 & good2 & good3;
   if (!good)   $display(" Error response source2 with %h to %h, good= %b%b%b",source2,destin2,good3,good2,good1);
   repeat (01)  @(posedge clock); #1;
   good     = 0; good1 = 0; good2 = 0; good3 = 0;

   // Clear it out so easier to see on waveform
   imosi    =  {3'b110,  32'h0, {1'b0,source1[7:0],4'h5}}; @(posedge clock); #1;
   imosi    =  {3'b110,  32'h0, {1'b0,source1[7:0],4'h6}}; @(posedge clock); #1;
   imosi    =  {3'b110,  32'h0, {1'b0,source1[7:0],4'h7}}; @(posedge clock); #1;
   imosi    =  {3'b110,  32'h0, {1'b0,source2[7:0],4'h5}}; @(posedge clock); #1;
   imosi    =  {3'b110,  32'h0, {1'b0,source2[7:0],4'h6}}; @(posedge clock); #1;
   imosi    =  {3'b110,  32'h0, {1'b0,source2[7:0],4'h7}}; @(posedge clock); #1;
   imosi    =  {48'h0};
   end
endtask

//task  cycle;
//input integer  number;
//   repeat (number) begin
//
//      for (i=0;i<8;i=i+1)
//         $write(" %h,%h  ",i_Q1.cols[i].i_col.iRW1.PRWI[0].iRP.Skid[071:064],
//                           i_Q1.cols[i].i_col.iRW1.PRWI[0].iRP.Pipe[071:064]);
//      $display("");
//
//      for (i=0;i<8;i=i+1)
//         $write(" %h,%h  ",i_Q1.cols[i].i_col.iRW1.PRWI[1].iRP.Skid[071:064],
//                           i_Q1.cols[i].i_col.iRW1.PRWI[1].iRP.Pipe[071:064]);
//      $display("");
//
//      for (i=0;i<8;i=i+1)
//         $write(" %h,%h  ",i_Q1.cols[i].i_col.iRW1.PRWI[2].iRP.Skid[071:064],
//                           i_Q1.cols[i].i_col.iRW1.PRWI[2].iRP.Pipe[071:064]);
//      $display("");
//
//      for (i=0;i<8;i=i+1)
//         $write(" %h,%h  ",i_Q1.cols[i].i_col.iRW1.PRWI[3].iRP.Skid[071:064],
//                           i_Q1.cols[i].i_col.iRW1.PRWI[3].iRP.Pipe[071:064]);
//      $display("");
//
//
////    for (i=0;i<9;i=i+1) begin $write(" %h  ",i_Q1.lane[i][263:256]); end $display("  => race[] = %h %h", i_Q1.cols[1].i_col.iRW1.PRWI[1].iRP.Skid[71:64], i_Q1.cols[0].i_col.iRW1.PRWI[2].iRP.Pipe[71:64]);
////    for (i=0;i<9;i=i+1) begin $write(" %h  ",i_Q1.lane[i][359:352]); end $display("  => race[] = %h %h", i_Q1.cols[1].i_col.iRW1.PRWI[0].iRP.Skid[71:64], i_Q1.cols[0].i_col.iRW1.PRWI[3].iRP.Pipe[71:64]);
//      temp[7:0] = i_Q1.cols[0].i_col.iRW1.RT[71:64];
//      tmp2[5:0] = {temp[6:4],temp[2:0]};
//      load_os   = 1;
//      nos       = os | (1 << tmp2);
//      $display("                                           ====>  %h    %d => %b", temp[7:0], tmp2, nos[63:0]);
//      @(posedge clock); #1;
//      os        = nos;
//      end
//endtask


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
