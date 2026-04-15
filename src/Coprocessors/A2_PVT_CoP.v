/* *******************************************************************************************************************************

   Copyright © 2025 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : COGNITUM

   Description          : PVT monitor interface

*********************************************************************************************************************************
   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*********************************************************************************************************************************

   Clock frequency needs to be between 1.15 MHz and 1.25 MHz and centered on 1.2 MHz
   A 4-bit command uses:
      Vsample Psample0 Psample1  EnA
         x        x        x     0     Reset
         0        0        0     1     Temperature Evaluation
         1        x        x     1     Voltage     Evaluation
         0        1        0     1     Process     Evaluation LVT
         0        0        1     1     Process     Evaluation SLVT
         0        1        1     1     Process     Evaluation RVT
   Evaluation is complete when the DataValid bit goes high.  This the 'Ready' register output
   2 trim control fit into the lower 2 bytes of a 'Trim' register
   Result data is read from a 'Result' register when ready

   1.15 MHz => 1 GHz = ~ x869  (1 Ghz /  869 = 1.150748)
   1.25 MHz => 1 GHz = ~ x800  (1 Ghz /  800 = 1.250000)
   1.15 MHz => 2 GHz = ~ x869  (1 Ghz / 1738 = 1.150748)
   1.25 MHz => 2 GHz = ~ x1600 (2 Ghz / 1600 = 1.250000)

   40:60 clock duty cycle required to final divide by 2 required.  So counter must be capable of supplying 2.3 .. 2.5 MHz

   12-bit counter followed by divide by 2

   Registers
      Frequency:        27:16  Count value
                        11:0  Taget value
      Command           21:16  TrimO[5:0]
                        12:8   TrimG[4:0]
                         3:0   {Vsample,Psample0,Psample1,EnA}
      Status            31:0   0 - Not ready   FFFFFFFF - Ready (Data Valid)
      Data               9:0   Data


********************************************************************************************************************************/

//JL Comment out this include construct to use final synthesis construct
//`ifdef   STANDALONE                    // DEFINE IN SIMULATOR COMMAND LINE
//   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src//include/A2_defines.vh"
//   `include "/proj/TekStart/lokotech/soc/users/roger/COGNITUM/cognitum_a0/src/include/COGNITUM_IO_addresses.vh" //JL
//`else
//   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src//include/A2_project_settings.vh"
//   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/COGNITUM_IO_addresses.vh" //JL
//`endif
//End Comment out

//JL - Use Romeo convention for Simulation vs Synthesis Include Configurations
`ifdef synthesis
     `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh" //JL - Don't use Romeo's version until later when Romeo synchronizes to latest GIT tip
     //`include "/proj/TekStart/lokotech/soc/users/jfechter/cognitum_a0/src/include/A2_project_settings.vh" //JL - Use presently checked in version until Romeo synchronizes to GIT Tip
`else
     `include "A2_defines.vh"
`endif
//End JL

module   A2_PVT_CoP #(
   parameter   [12:0]   BASE     = 13'h00000,
   parameter            NUM_REGS = 4
   )(
   // A2S I/O interface
   output wire [32:0]   imiso,   // Slave data out {               IOk, IOo[31:0]}
   input  wire [47:0]   imosi,   // Slave data in  {cIO[2:0] aIO[12:0], iIO[31:0]}
   // PVT interface                                 15  14:10      9:4        3   2       1:0
   output wire [15:0]   pmosi,   // PVT inputs     {CLK,TRIMG[4:0],TRIMO[5:0],ENA,VSAMPLE,PSAMPLE[1:0],}
   input  wire [10:0]   pmiso,   // PVT outputs    {DATA_VALID,DATA_OUT[9:0]}
   // Global
   output wire          intpt,   // Evaluation complete interrupt -- cleared by taking ENA low
   input  wire          reset,
   input  wire          clock
   );

localparam  LG2REGS  =  `LG2(NUM_REGS);
//JL - Use Romeo convention for Simulation vs Synthesis Include Configurations
`ifdef synthesis
	 `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/COGNITUM_IO_addresses.vh" //Comment out until Romeo ready for synthesis of compilable Top Netlist
	 //`include "/proj/TekStart/lokotech/soc/users/jfechter/cognitum_a0/src/include/COGNITUM_IO_addresses.vh" //Use local repo copy until Romeo's directory is updated to repo tip
`else
	`include "COGNITUM_IO_addresses.vh"
`endif
//End JL

// Physical Registers ==========================================================================================================

reg       [31:0]  FREQ, CMND;
reg               DIV2;

// psuedo registers
wire      [31:0]  status, data;

// Interface decoding -----------------------------------------------------------------------------------------------------------

wire  [31:0]   iIO, IOo;
wire  [12:0]   aIO;
wire  [ 2:0]   cIO;
// Decollate imosi
assign   {cIO, iIO, aIO} = imosi;

wire                  in_range =  aIO[12:LG2REGS]  == BASE[12:LG2REGS];
wire  [NUM_REGS-1:0]    decode =  in_range << aIO[LG2REGS-1:0];
wire  [NUM_REGS-1:0]     write =  {NUM_REGS{(cIO==3'b110)}} & decode;
wire  [NUM_REGS-1:0]      read =  {NUM_REGS{(cIO==3'b101)}} & decode;

// Writes
wire  ld_F  =  write[PVT_FREQ   ],      //
      ld_C  =  write[PVT_CMND   ];      //JL

// Reads
wire  rd_F  =  read[PVT_FREQ   ],      //
      rd_C  =  read[PVT_CMND   ],      //
      rd_S  =  read[PVT_STATUS ],      //
      rd_D  =  read[PVT_DATA   ];      //JL

wire  IOk   = |write[NUM_REGS-1:0] |  |read[NUM_REGS-1:0];

assign  IOo   =  rd_F         ?  FREQ   //JL
            :  rd_C           ?  CMND
            :  rd_S           ?  status
            :  rd_D           ?  data   : 32'h00000000;

assign
      imiso = {IOk, IOo};

// Registers

wire  limit = FREQ[27:16] == FREQ[11:0];

always @(posedge clock)
   if      (ld_F)                FREQ        <= `tCQ  iIO;
   else if (FREQ[31] &&  limit)  FREQ[27:16] <= `tCQ  12'b0;
   else if (FREQ[31] && !limit)  FREQ[27:16] <= `tCQ  FREQ[27:16] + 1'b1;

always @(posedge clock)
   if       (reset)  DIV2  <= `tCQ  1'b0;
   else if  (limit)  DIV2  <= `tCQ ~DIV2;


always @(posedge clock)
   if      (ld_C)    CMND  <= `tCQ  iIO;

// PVT Interface
//                 15    14:10      9:4        3   2       1:0
//                {CLK,  TRIMG[4:0],TRIMO[5:0],ENA,VSAMPLE,PSAMPLE[1:0],}
assign   pmosi =  {DIV2, CMND[14:0]};

assign   data  =  {22'h0,pmiso[9:0]};

assign   status=  {32{pmiso[10]}};

assign intpt = pmiso[10]; //JLPL - 11_4_25

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

`ifdef   verify_PVT                 // DEFINE IN SIMULATOR Command Line

module   t;

parameter   [12:0]   BASE     = 13'h00000,
parameter            NUM_REGS = 4

reg  [47:0]   imosi,   // Slave data in  {cIO[2:0] aIO[12:0], iIO[31:0]}
reg  [10:0]   pmiso,   // PVT outputs    {DATA_VALID,DATA_OUT[9:0]}
reg           reset,
reg           clock

wire [32:0]   imiso,   // Slave data out {               IOk, IOo[31:0]}
wire [15:0]   pmosi,   // PVT inputs     {CLK,TRIMG[4:0],TRIMO[5:0],ENA,VSAMPLE,PSAMPLE[1:0],}
wire          intpt,   // Evaluation complete interrupt -- cleared by taking ENA low


A2_PVT_CoP #(BASE,NUM_REGS) mut (
   .imiso(imiso),   // Slave data out {               IOk, IOo[31:0]}
   .imosi(imosi),   // Slave data in  {cIO[2:0] aIO[12:0], iIO[31:0]}
   .pmosi(pmosi),   // PVT inputs     {CLK,TRIMG[4:0],TRIMO[5:0],ENA,VSAMPLE,PSAMPLE[1:0],}
   .pmiso(pmiso),   // PVT outputs    {DATA_VALID,DATA_OUT[9:0]}
   .intpt(intpt),   // Evaluation complete interrupt -- cleared by taking ENA low
   .reset(reset),
   .clock(clock)
   );

initial begin
   clock =  0;
   reset =  0;
   imosi =  0;
   pmiso =  0;
   $finish();
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
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

