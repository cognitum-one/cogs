//===============================================================================================================================
//
//    Copyright © 2025 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//
//    Project Name   : COGNITUM NEWS DMA
//
//    Description    : Pipe based DMA from Tile Zero Memory to the NEWS network
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
//    31                15               0
//    +--+-----+-----+--+--+-----+-----+--+
//    |00|   limit   |00|00|   base    |0E|           DMA Control Register. One for each direction
//    +--+-----+-----+--+--+-----+-----+--+
//              \                 \       \__________ Enable DMA
//               \                 \_________________ Bottom address of circular buffer
//                \__________________________________ Upper  address of circular buffer
//
//==============================================================================================================================

`ifndef  tCQ
`define  tCQ   #1
`endif

module NEWS_DMA (
   // Pipe Output Interface
   output wire          LOor,          // Local Output to NEWS  -- output ready
   output wire [31:0]   LOdo,          // Local Output to NEWS  -- data output
   input  wire          LOrd,          // Local Output to NEWS  -- read (take)
   // Pipe Input Interface
   output wire          LIir,          // Local Input from NEWS -- Input Ready
   input  wire [31:0]   LIdi,          // Local Input from NEWS -- Input data
   input  wire          LIwr,          // Local Input from NEWS -- write (give)
   // Memory Interface                 // Memory is a slave
   output wire [66:0]   lmosi,         // // 67 {breq, bwr,brd,  bwrd[31:0], badr[31:0]} //JLPL - 8_18_25
   input  wire [33:0]   lmiso,         //      {lgnt, lack, lrdd[31:0]}  = 34 //JLPL - 8_21_25
   // DMA Outgoing Complete INTERRUPT  //JLPL - 9_6_25
   output wire          intpt_ldma,    // JLPL - 9_6_25 - DMA Outgoing Transfer Complete interrupt
   // Register Interface
   //output wire [31:0]   ndmao,         // These should merge into imosi/imiso //JLPL - 8_18_25
   output  wire [31:0]  read_incoming, //JLPL - 8_18_25
   output  wire [31:0]	read_outgoing, //JLPL - 8_18_25
   output  wire [31:0]	read_state,    //JLPL - 8_18_25
   input  wire [31:0]   ndmai,         //
   input  wire [ 1:0]   reg_write,     // decoded write
   //input  wire [ 1:0]   reg_read,      // decoded read //JLPL - 8_18_25
   // Global
   input  wire          clock,
   input  wire          reset
   );

localparam     L_NEWS_L_DMA_INCOMING = 0,
               L_NEWS_L_DMA_OUTGOING = 1,
               L_NEWS_L_DMA_STATE    = 2;

// Registers
reg   [31:0]   NLDI,                   // NEWS LOCAL DMA INPUT-SIDE  SETTINGS
               NLDO;                   // NEWS LOCAL DMA OUTPUT-SIDE SETTINGS //JLPL - 8_18_25
reg   [13:2]   NLDIdma,                // NEWS LOCAL DMA INPUT  ADDRESS
               NLDOdma;                // NEWS LOCAL DMA OUTPUT ADDRESS

// Locals
wire  [ 1:0]   wrap, advnc, start;

// Memory Interface
wire  [31:0]   lrdd;                   // read-data
wire           lack, lgnt;             // acknowledge, grant
wire  [31:0]   Ladr, Lwrd;             // Address, Write-data //JLPL - 8_18_25
wire           Lreq, Lwr;              // Request, Write //JLPL - 8_18_25

// A2S I/O Interface ============================================================================================================
// Collate
assign    lmosi[66:0]   =  {Lreq,Lwr,~Lwr,Lwrd[31:0],Ladr[31:0]};         // 1+1+1+32+32   =   67 //JLPL - 8_18_25 - 8_24_25
// Decollate
assign   {lgnt,lack,lrdd[31:0]}  =  lmiso[33:0];                     // 1+1+32      =   34  //JLPL - 8_21_25

// Registers
always @(posedge clock)
   if      (reset)                            NLDI        <= `tCQ  32'h0; //JLPL - 8_19_25
   else if (reg_write[L_NEWS_L_DMA_INCOMING]) NLDI        <= `tCQ   ndmai[31:0]; //JLPL - 8_19_25
   else if (    start[L_NEWS_L_DMA_INCOMING]) NLDI[ 1:0 ] <= `tCQ  {NLDI[0], 1'b0};
   else if (     wrap[L_NEWS_L_DMA_INCOMING]) begin                                   //JLPL - 8_20_25
                                              NLDI[ 1:0 ] <= `tCQ  2'b00;             // Save buffer wraparound incidents
                                              NLDI[31:30] <= `tCQ  {1'b1,NLDI[31]};
                                              end

always @(posedge clock)
   if      (reset)                            NLDO        <= `tCQ  32'h0; //JLPL - 8_19_25
   else if (reg_write[L_NEWS_L_DMA_OUTGOING]) NLDO        <= `tCQ   ndmai[31:0]; //JLPL - 8_19_25
   else if (    start[L_NEWS_L_DMA_OUTGOING]) NLDO[ 1:0 ] <= `tCQ  {NLDO[0], 1'b0};
   else if (     wrap[L_NEWS_L_DMA_OUTGOING]) begin                                   //JLPL - 8_20_25
                                              NLDO[ 1:0 ] <= `tCQ  2'b00;             // Save buffer wraparound incidents
                                              NLDO[31:30] <= `tCQ  {1'b1,NLDO[31]};
                                              end

// Register readback - JLPL - 8_18_25
//assign
//   ndmao[31:0] =  reg_read[L_NEWS_L_DMA_INCOMING]   ?  NLDI[31:0]
//               :  reg_read[L_NEWS_L_DMA_OUTGOING]   ?  NLDO[31:0]
//               :  reg_read[L_NEWS_L_DMA_STATE       ? {NLDOdma, NLDIdma}
//               :                                        32'h0;
assign read_incoming = NLDI[31:0]; //JLPL - 8_18_25
assign read_outgoing = NLDO[31:0]; //JLPL - 8_18_25
assign read_state = {NLDOdma, NLDIdma};    //JLPL - 8_18_25

// Pipes ========================================================================================================================

wire [31:0] LIdo; //JLPL - 8_18_25
wire LIor; //JLPL - 8_18_25
wire LIrd; //JLPL - 8_18_25
wire [31:0] LOdi; //JLPL - 8_18_25
wire LOir; //JLPL - 8_18_25
wire LOwr; //JLPL - 8_18_25

A2_pipe #(32) i_LI (    // Local input from NEWS switch => Writes to Memory
   // Pipe Output to RAM
   .pdo  (LIdo),
   .por  (LIor),        // Creates write access
   .prd  (LIrd),        // POPs write data on grant
   // Pipe Input
   .pdi  (LIdi),
   .pir  (LIir),
   .pwr  (LIwr),
   .clock(clock),
   .reset(reset)
   );

A2_pipe #(32) i_LO (    // Reads from Memory => Local output to NEWS switch
   // Pipe Output
   .pdo  (LOdo),
   .por  (LOor),
   .prd  (LOrd),
   // Pipe Input to RAM
   .pdi  (LOdi),
   .pir  (LOir),        // Creates read access
   .pwr  (LOwr),        // Pushes new data on acknowledge
   .clock(clock),
   .reset(reset));


// Address Counters =============================================================================================================

assign
   start[L_NEWS_L_DMA_INCOMING] =   NLDI[0],
   start[L_NEWS_L_DMA_OUTGOING] =   NLDO[0]; //JLPL - 8_18_25

always @(posedge clock)
   if      (start[L_NEWS_L_DMA_INCOMING]) NLDIdma  <= `tCQ  NLDI[13:2];                // Load   base address of NLDI
   else if ( wrap[L_NEWS_L_DMA_INCOMING]) NLDIdma  <= `tCQ  NLDI[13:2];                // Reload base address of NLDI
   else if (advnc[L_NEWS_L_DMA_INCOMING]) NLDIdma  <= `tCQ  NLDIdma + 1'b1;            // Advance address by 4 bytes

always @(posedge clock)
   if      (start[L_NEWS_L_DMA_OUTGOING]) NLDOdma  <= `tCQ  NLDO[13:2];                // Load base address of NLDO
   else if ( wrap[L_NEWS_L_DMA_OUTGOING]) NLDOdma  <= `tCQ  NLDO[13:2];                // Reload base address of NLDO
   else if (advnc[L_NEWS_L_DMA_OUTGOING]) NLDOdma  <= `tCQ  NLDOdma + 1'b1;            // Advance address by 4 bytes

// State Machine ================================================================================================================
// Local traffic is from only one cardinal direcetion at-a-time.  So best case of 4 Gb/s 8b|10b is 400 MB/s. Local transfer is on
// 4 byte boundaries to the maximum rate is 100 MW/s or 1 cycle in 10 peak

reg         [2:0] FSM;
localparam  [2:0] IDLE = 0,  RDLAST = 1, WRLAST = 2, READ = 3, WRITE = 4, RDACK = 5, WRACK = 6; //JLPL - 8_21_25
wire                         rdlast,     wrlast,     read,     write,     rdack,     wrack; //JLPL - 8_21_25

always @(posedge clock)
   if (reset)  FSM <= `tCQ IDLE;
   else (* parallel_case *) case(FSM)
      default  if ( LIor && NLDI[1]) FSM <= `tCQ WRITE;         // IDLE case!  Writes have precedence to minimize NEWS back-ups //JLPL - 8_20_25
          else if ( LOir && NLDO[1]) FSM <= `tCQ READ;          //JLPL - 8_20_25
      WRLAST:  if ( wrap[L_NEWS_L_DMA_INCOMING] )  FSM <= `tCQ IDLE; //JLPL - 8_21_25
	      else if ( LOir && NLDO[1]) FSM <= `tCQ READ;          // Update dma address //JLPL - 8_21_25
          else if ( LIor && NLDI[1]) FSM <= `tCQ WRITE;         // If we get busy we alternate! //JLPL - 8_20_25
          else             FSM <= `tCQ IDLE;
      RDLAST:  if ( wrap[L_NEWS_L_DMA_OUTGOING] )  FSM <= `tCQ IDLE; //JLPL - 8_21_25
	      else if ( LIor && NLDI[1]) FSM <= `tCQ WRITE;         // Update dma address //JLPL - 8_21_25
          else if ( LOir && NLDO[1]) FSM <= `tCQ READ;          // If we get busy we alternate! //JLPL - 8_20_25
          else             FSM <= `tCQ IDLE;
      WRITE :  if ( lgnt ) FSM <= `tCQ WRACK; //JLPL - 8_21_25
      READ  :  if ( lgnt ) FSM <= `tCQ RDACK; //JLPL - 8_21_25
      WRACK :  if ( lack ) FSM <= `tCQ WRLAST;        // Wait for Acknowledge
      RDACK :  if ( lack ) FSM <= `tCQ RDLAST;
      endcase

assign
   read     =  FSM == READ,
   write    =  FSM == WRITE,
   rdack    =  FSM == RDACK,
   wrack    =  FSM == WRACK,
   wrlast   =  FSM == WRLAST,
   rdlast   =  FSM == RDLAST,

   Lreq     =  write | read,
   Lwr      =  write,

   Lwrd     =  LIdo[31:0],
   LOdi     =  lrdd[31:0],
   Ladr     =  write ? {16'h8000,2'b00,NLDIdma[13:2],2'b00} //JLPL - 8_22_25
            :  read  ? {16'h8000,2'b00,NLDOdma[13:2],2'b00} //JLPL - 8_22_25
            :           32'h0;
assign   LOwr     =  rdack   & lack; //JLPL - 8_18_25
assign   LIrd     =  write & lgnt; //JLPL - 8_21_25

assign
   advnc[L_NEWS_L_DMA_INCOMING]  =  wrlast,
   advnc[L_NEWS_L_DMA_OUTGOING]  =  rdlast,
   wrap [L_NEWS_L_DMA_INCOMING]  =  wrlast  &&  NLDIdma[13:2] == NLDI[29:18],
   wrap [L_NEWS_L_DMA_OUTGOING]  =  rdlast  &&  NLDOdma[13:2] == NLDO[29:18]; //JLPL - 8_18_25
   
assign intpt_ldma = NLDO[31];    // JLPL - 9_6_25 - DMA Outgoing Transfer Complete interrupt

endmodule



////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

