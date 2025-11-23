//===============================================================================================================================
//
//    Copyright © 2025 Advanced Architectures
//
//       All rights reserved
//       Confidential Information
//       Limited Distribution to Authorized Persons Only
//       Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//       Project Name         : Newport
//
//       Description          : TileZero -- AES Crypto processor
//
//===============================================================================================================================
// THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
// BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
// TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
// AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
//===============================================================================================================================
//    REGISTER MAP [relative to RoPUF 39-bit data+ecc_check_bits ]
//
//    KEYSTORE Register file (64)                              Initialization Vector and Counter Register File (64)
//    ISKTON    0..3    Interface  Session Key To North        IVCTON   64..67   IVC To North
//    ISKTOS    4..7    Interface  Session Key To South        IVCTOS   68..71   IVC To South
//    ISKTOE    8..11   Interface  Session Key To East         IVCTOE   72..75   IVC To East
//    ISKTOW   12..15   Interface  Session Key To West         IVCTOW   76..79   IVC To West
//    ISKFRN   16..19   Interface  Session Key From North      IVCFRN   80..83   IVC From North
//    ISKFRS   20..23   Interface  Session Key From South      IVCFRS   84..87   IVC From South
//    ISKFRE   24..27   Interface  Session Key From East       IVCFRE   88..91   IVC From East
//    ISKFRW   28..31   Interface  Session Key From West       IVCFRW   92..95   IVC From West
//    ISK0FR   32..35   End-to-End Session Key-0 From          IVC0FR   96..99   IVC-0 From End-to-End
//    ISK0TO   36..39   End to End Session Key-0 To            IVC0TO  100..103  IVC-0 To   End to End
//    ISK1FR   40..43   End-to-End Session Key-1 From          IVC1FR  104..107  IVC-1 From End-to-End
//    ISK1TO   44..47   End to End Session Key-1 To            IVC1TO  108..111  IVC-1 To   End to End
//    ISKMLT   48..51   Multicast  Session Key                 IVCMLT  112..115  IVC Multicast
//    ISKSP0   52..55   Spare 0                                IVCSP0  116..119  Spare 0
//    ISKSP1   56..59   Spare 1                                IVCSP1  120..123  Spare 1
//    ISKSP2   60..63   Spare 2                                IVCSP2  124..127  Spare 2
//
//    OBFUSC 128..131   Obfuscation Register [OBF]  (RoT only, not readable over imosi/imiso))
//
//===============================================================================================================================
//    REGISTER MAP [relative to imosi base address] <256 addresses: Base address MUST end in 8'h00!
//
//    KEYSTORE Register file (53)                              Initialization Vector and Counter Register File (52)
//    ISKTON    0..3    Interface  Session Key To North        IVCTON   64..67   IVC To North
//    ISKTOS    4..7    Interface  Session Key To South        IVCTOS   68..71   IVC To South
//    ISKTOE    8..11   Interface  Session Key To East         IVCTOE   72..75   IVC To East
//    ISKTOW   12..15   Interface  Session Key To West         IVCTOW   76..79   IVC To West
//    ISKFRN   16..19   Interface  Session Key From North      IVCFRN   80..83   IVC From North
//    ISKFRS   20..23   Interface  Session Key From South      IVCFRS   84..87   IVC From South
//    ISKFRE   24..27   Interface  Session Key From East       IVCFRE   88..91   IVC From East
//    ISKFRW   28..31   Interface  Session Key From West       IVCFRW   92..95   IVC From West
//    ISK0FR   32..35   End-to-End Session Key-0 From          IVC0FR   96..99   IVC-0 From End-to-End
//    ISK0TO   36..39   End to End Session Key-0 To            IVC0TO  100..103  IVC-0 To   End to End
//    ISK1FR   40..43   End-to-End Session Key-1 From          IVC1FR  104..107  IVC-1 From End-to-End
//    ISK1TO   44..47   End to End Session Key-1 To            IVC1TO  108..111  IVC-1 To   End to End
//    ISKMLC   48..51   Interface  Session Key Multicast       IVCMLC  112..115  IVC Multicast
//    ISKSP0   52..55   Spare 0                                IVCSP0  116..119  Spare 0
//    ISKSP1   56..59   Spare 1                                IVCSP1  120..123  Spare 1
//    ISKSP2   60..63   Spare 2                                IVCSP2  124..127  Spare 2
//
//    These are only 7 bits wide_______________________________________ & these are only 7 bits wide_____ & occupy lower 2 bytes
//    ISKTNC  128..131  Interface Session Key To North   Checkbits      IVC To North           Checkbits
//    ISKTSC  132..135  Interface Session Key To South   Checkbits      IVC To South           Checkbits
//    ISKTEC  136..139  Interface Session Key To East    Checkbits      IVC To East            Checkbits
//    ISKTWC  140..143  Interface Session Key To West    Checkbits      IVC To West            Checkbits
//    ISKFNC  144..147  Interface Session Key From North Checkbits      IVC From North         Checkbits
//    ISKFSC  148..151  Interface Session Key From South Checkbits      IVC From South         Checkbits
//    ISKFEC  152..155  Interface Session Key From East  Checkbits      IVC From East          Checkbits
//    ISKFWC  156..159  Interface Session Key From West  Checkbits      IVC From West          Checkbits
//    ISK0FC  160..163  Interface Session Key-0 From E2E Checkbits      IVC-0 From End-to-End Checkbits
//    ISK0TC  164..167  Interface Session Key-0 To   E2E Checkbits      IVC-0 To   End to End Checkbits
//    ISK1FC  168..171  Interface Session Key-1 From E2E Checkbits      IVC-1 From End-to-End Checkbits
//    ISK1TC  172..175  Interface Session Key-1 To   E2E Checkbits      IVC-1 To   End to End Checkbits
//    ISKMLC  176..179  Interface Session Key Multicast  Checkbits      IVC Multicast         Checkbits
//    ISKS0C  180..183  Spare 0                                         Spare 0
//    ISKS1C  184..187  Spare 1                                         Spare 1
//    ISKS2C  188..191  Spare 2                                         Spare 2
//
//    CSR          192  Control Status Register
//       bit 0:         Enable AES     -- State Machine held in IDLE if this bit is zero
//       bit 1:         Double Error   -- Double Error requires a reload of Keys and IVCs  -- cleared when done by SW
//                                        also forces FSM to IDLE when rolled back!
//       bit 2:         On GSM H generation 0: disable  1: enable IVC increment
//
//===============================================================================================================================
//  Requests from NEWS
//    AES :  0_000    AES From North
//    AES :  0_001    AES From East
//    AES :  0_010    AES From West
//    AES :  0_011    AES From South
//    AES :  0_100    AES  To  North
//    AES :  0_101    AES  To  East
//    AES :  0_110    AES  To  West
//    AES :  0_111    AES  To  South
//  Requests from GCM
//    GCM :  1_000    Session 0 From H      AES(Key,0)
//    GCM :  1_001    Session 0 From E      AES(Key,(IV||++C)     Implies Counter part of IVC must be loaded with minus 1
//    GCM :  1_010    Session 0 To   H      AES(Key,(IV||++C)     Counter >= 1
//    GCM :  1_011    Session 0 To   E
//    GCM :  1_100    Session 1 From H      Nota Bene: AES sees Ez and En as the same.  GCM uses this code
//    GCM :  1_101    Session 1 From E                 to steer incoming data appropriately
//    GCM :  1_110    Session 1 To   H
//    GCM :  1_111    Session 1 To   E
//===============================================================================================================================

//`ifdef synthesis //JLSW
//    `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2_project_settings.vh"
//    `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/NEWPORT_defines.vh"
//`else
//`include "A2_project_settings.vh"
//`include "NEWPORT_defines.vh"
//`endif
//End JL


module   A2_AES_CoP #(
   parameter   [12:0]   BASE = 13'h00000, // Base Address of Co-processor must end in 8'h00!!
   parameter        NUM_REGS = 256        // Not all equipped! < see above >
   )(
   // AES Interface
   output wire [40:0]   amiso,         // AES output           41 {ECBor, ECBad[3:0], ECBpo[3:0], ECB[31:0]}
   input  wire [ 1:0]   amiso_rd,      // Read / Take           2 {ECBrd[1:0]} bit0 = NEWS, bit1 = GCM
   // NEWS Requests
   input  wire [ 5:0]   nmosi,         // NEWS request/grant,   4 {AESwr, AESreq[4:0]} //JLPL - 10_17_25
   output wire          nmosi_fb,      // Input ready           1 {AESir}
   // GCM  Requests
   input  wire [ 5:0]   gmosi,         // Read and consume      4 {AESwr, AESreq[4:0]} //JLPL - 10_17_25
   output wire          gmosi_fb,      // Input ready           1 {AESir}
   // Root of Trust Interface
   input       [47:0]   rmosi,         // Root-of-Trust        48 {RoTload, RoTaddr[7:0], RoTchk[6:0], RoTdata[31:0]}
   // A2S IO Interface
   output wire [32:0]   imiso,         // Slave data out       32 {                IOk, IOo[31:0]} //JLSW
   input  wire [47:0]   imosi,         // Slave data in        48 {cIO[2:0], iIO[31:0], aIO[12:0]}
   // Global
   output wire          intpt,         // flag A2S: error      Double-bit error in ECC for IVC and KEY stores //JLPL - 10_17_25
   input  wire          reset,
   input  wire          clock
   );

//JL - Use Romeo convention for Simulation vs Synthesis Include Configurations
`ifdef synthesis
    `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/NEWPORT_IO_addresses.vh" //Comment out until Romeo ready for synthesis of compilable Top Netlist
    //`include "/proj/TekStart/lokotech/soc/users/jfechter/newport_a0/src/include/NEWPORT_IO_addresses.vh" //Use local repo copy until Romeo's directory is updated to repo tip
`else
   `include "NEWPORT_IO_addresses.vh"
`endif
//End JL

localparam  LNRG  =  `LG2(NUM_REGS);
genvar      i;

// Physical Registers ===========================================================================================================

reg   [79:0]   RF    [0:63];     // Encryption Keys siamesed with Initialization Vectors (Nonces) and Counters all with checkbits
reg   [38:0]   OBF   [0: 3];     // Obfuscation with checkbits
//reg            Carry;            // Save Carry bit for Counter increment //JLSW

// Collate Outputs & Decollate Inputs ===========================================================================================

// AES Output Interface ---------------------------------------------------------------------------------------------------------
wire  [31:0]   ECBo;          // Encrypted Cipher Block Word
wire  [ 3:0]   ECBpo;         // Encrypted Cipher Block byte parity
wire  [ 3:0]   ECBad;         // Encrypted Cipher Block destination address
wire           ECBlast;
wire           ECBor;   //JL - Move earlier in the code
reg   [79:0]   RFo;     //JL - Declare earlier in code //JLPL - 9_18_25 - Change to reg

reg            Cmnd;    //JLPL - 10_17_25

reg   [31:0]   CSR;     //JLSW
reg   [ 3:0]   Cycle;   //JLSW - Declare earlier in code for this compiler
reg            wr_pipe;
wire           fail;    //JLPL Missing wire definition - 7_29_25


// Collate output
assign         amiso[40:0] =  {ECBor, ECBad[3:0], ECBpo[3:0], ECBo[31:0]};

// RoT Interface ----------------------------------------------------------------------------------------------------------------
wire  [31:0]   RoTdata;
wire  [ 6:0]   RoTchkb;
wire  [ 7:0]   RoTaddr;
wire           RoTload;
// Decollate input
assign        {RoTload, RoTaddr[7:0], RoTchkb[6:0], RoTdata[31:0]} = rmosi;

// Load OBF register
always @(posedge clock)
   if (RoTload && RoTaddr[7:2] == 6'b100000) OBF[RoTaddr[1:0]][38:0] <= `tCQ {RoTchkb[6:0],RoTdata[31:0]};

// A2S IO Interface -------------------------------------------------------------------------------------------------------------
wire  [ 2:0]   cIO;
wire  [31:0]   IOo, iIO;
wire  [12:0]   aIO;
wire           IOk; //JLPL
// Decollate input
assign        {cIO[2:0],iIO[31:0],aIO[12:0]} =  imosi[47:0]; //JLPL - 10_17_25

// Collate output
assign         imiso = {IOk, IOo[31:0]};

wire  [31:0]   jIO   =  iIO[31:0] ^    OBF[aIO[1:0]][31:00];   // Obfuscate 4 bytes
wire  [13:0]   kIO   =  iIO[13:0] ^ {2{OBF[aIO[1:0]][38:32]}}; // Obfuscate 2 bytes //JLPL - 10_15_25 - Roger's addressing change for GCM

// Locals -----------------------------------------------------------------------------------------------------------------------
wire  [ 1:0]   SEC,  DED;
wire           idle, read, increment, update;

wire  [ 38:0]  IVCincremented;
wire  [ 79:0]  ERRfixed; //JLSW
wire  [ 79:0]  nRF;
wire  [127:0]  iSK,  iIVC;
wire  [ 15:0]  iSKp, iIVCp;
wire  [127:0]  AESo;
wire  [ 15:0]  AESop;
wire           AESrd; //JLSW
wire  [  4:0]  request_addr; //JLPL - 10_17_25
wire           request_give;
wire           request_take;
wire           request_read = idle;
wire           carry_out; //JLPL - 10_15_25 - Add missing wire declaration

wire           DESTir; //JLPL - 8_1_25
wire           da1_ordy; //JLPL - 8_1_25
wire  [  3:0]  da1;      //JLPL - 8_1_25
wire           da1_read; //JLPL - 8_1_25

// Physical Registers ===========================================================================================================

reg   [77:0]   KEY   [0:63];     // Encryption Keys with checkbits
reg   [77:0]   IVC   [0:63];     // Initialization Vectors (Nonces) and Counters with checkbits
//reg   [38:0]   OBF   [0: 3];     // Obfuscation with checkbits //JL
//reg            Carry;            // Save Carry bit for Counter increment //JL

// A2S I/O Interface decoding ===================================================================================================

wire
   in_range =  cIO[2]   &  aIO[12:LNRG] == BASE[12:LNRG],      // Detect address to this module
   in_IVC   =  in_range & ~aIO[LNRG-1] & ~aIO[LNRG-2],         // IVC register file data
   in_KEY   =  in_range & ~aIO[LNRG-1] &  aIO[LNRG-2],         // KEY register file data
   in_CHK   =  in_range &  aIO[LNRG-1] & ~aIO[LNRG-2],         // CHK register file ECC check bits
   in_CSR   =  in_range & (aIO[LNRG-1:0] == 8'd192),
   in_BOTH  =  in_IVC   |  in_KEY, //JLPL - 7_30_25
   in_range_below_CSR = in_range & ( ~aIO[LNRG-1] | ~aIO[LNRG-2] ); //JLPL - 7_30_25 per discussions with Roger

wire
   wr       =  in_range & (cIO[1:0] == 2'b10),
   rd       =  in_range & (cIO[1:0] == 2'b01);

always @(posedge clock)
   if       (reset      )  CSR      <= `tCQ  32'h0;
   else  if (in_CSR & wr)  CSR      <= `tCQ  iIO[31:0];
   else  if (fail       )  CSR[1]   <= `tCQ  1'b1;

assign   intpt  =  CSR[1]; //JLPL - 10_17_25

//always @(posedge clock) if (in_range)  IOk   <= `tCQ  in_range  & cIO[2]; //JLPL
assign  IOk  =  wr | rd; //JLPL

A2_mux #(4,32) iIOO (.z(IOo[31:0]),
                     .s({in_CSR, in_CHK, in_IVC, in_KEY}),
                     .d({CSR, 16'h0, RFo[79:72], RFo[39:32], RFo[63:32], RFo[31:0]})); //JLSW

// Register File ================================================================================================================
// Only a single Register File  KEY and IVC are siamesed and accessed together
// IVC Store side
// On each read of a value in IVC the lower half [63:0] are incremented.
// So the check bits must also update to match.
// The a2_ecc32_incrementer does that predictively rather than regenerating.
// The carry-out bit 32 is also needed to correctly update the check bits for [63:32].
//===============================================================================================================================
//
//    79 78  72 71    64 63    56 55    48 47    40   38  32 31    24 23    16 15     8 7      0
//    +-+------+--------+--------+--------+--------+-+------+--------+--------+--------+--------+
//    |0| CHKI |                IVC                |0| CHKK |                KEY                |   as held in Register File
//    +-+------+--------+--------+--------+--------+-+------+--------+--------+--------+--------+
//
//    39 and 79 are set on single-bit error detect.
//===============================================================================================================================

wire        KEYsec, KEYded;
wire        IVCsec, IVCded;

wire        eRF   =  RoTload                                                  // Initial Load from RoPUF
                  |  in_range_below_CSR                                       // IMOSI access //JLPL - 7_30_25 was in_range
                  |  update                                                   // JLPL - 7_30_25 per discussions with Roger
                  |  read;                                                    // Read access for AES computation

wire        wRF   =  RoTload  | update | in_range_below_CSR & wr;             // Writes //JLPL - 7_30_25 was in_range

wire  [5:0] aRF   =  RoTload              ?  RoTaddr[5:0]                     //
                  :  in_range_below_CSR   ?      aIO[5:0]                     //JLPL - 7_30_25 was in_range
                  :                            {request_addr[3:0],~Cycle[1:0]};   // NEWS OR GCM request //JLPL - 10_17_25 - Now same addressing for NEWS & GCM

wire [79:0] iRF   =  RoTload  ? {2{1'b0,RoTchkb[6:0], RoTdata[31:0]}}         // Write one-side, mask dependent
                  :  in_BOTH  ? {2{8'h0,      jIO[31:0]}}                     // Write one-side, mask dependent
                  :  in_CHK   ? {kIO[13:7], 33'h0, kIO[6:0], 33'h0}           // Write both sets of checkbits together //JLPL - 10_15_25
                  :              nRF[79:0];                                   // Write both simultaneously

wire [79:0] mRF   =  RoTload && !RoTaddr[6]  ?  80'h00_0000_0000_ff_ffff_ffff // RMOSI write access
                  :  RoTload &&  RoTaddr[6]  ?  80'hff_ffff_ffff_00_0000_0000 // RMOSI write access
                  :  in_IVC                  ?  80'h00_ffff_ffff_00_0000_0000 // IMOSI write access
                  :  in_KEY                  ?  80'h00_0000_0000_00_ffff_ffff // IMOSI write access
                  :  in_CHK                  ?  80'hff_0000_0000_ff_0000_0000 // IMOSI write access
                  :                             80'hff_ffff_ffff_ff_ffff_ffff;// updates and increments


// KEY Register File with bitmask //JLPL - 9_18_25 Change to discrete mask RAM with proper polarity signals
//reg   [79:0]   RF [0:63];
wire  [79:0]   mRFn  =  ~mRF[79:0];
wire  eRFn  =  ~eRF, wRFn  =  ~wRF;
integer j;

always @(posedge clock)
begin
   if (!eRFn) 
	begin
		if (!wRFn)
			begin
				for (j=0; j<80; j=j+1)
				  begin
					if (!mRFn[j])
						RF[aRF[5:0]][j] <= `tCQ  iRF[j];
				  end //end for j
			end //end if !wRFn
		else
			RFo   <=  `tCQ RF[aRF[5:0]];
	end //end if !eRFn
end //end always

//mRAM #(
//   .DEPTH(64),
//   .WIDTH(80)
//   ) i_RFK (
//   .RAMo (RFo),   // Data Output /* MULIT-CYCLE NET */
//   .iRAM (iRF),   // Data Input
//   .mRAM (mRF),   // Write Mask per word
//   .aRAM (aRF),   // Address
//   .eRAM (eRF),   // Enable
//   .wRAM (wRF),   // Write
//   .clock(clock)
//   );

// A pair of ECC decoders
for (i=0; i<2; i=i+1) begin : A
   A2_ecc32_dec i_secded (
      .Co  (ERRfixed[40*i+32+:7]),  // Output 7 Check bits & 32 bit data bits //JLSW
      .Do  (ERRfixed[40*i+:32]),    //JLSW
      .sec (SEC[i]),                // Output Single error corrected event       => FSM input
      .ded (DED[i]),                // Output Double error detected  event       => FSM input
      .iD  (RFo[40*i+:39])          // Input  38 bit Protected input data to be checked and corrected and output
      );
   end

assign
   {IVCsec,KEYsec} = SEC[1:0],
   {IVCded,KEYded} = DED[1:0]; //JLSW
// Increment the counter part of IVC, after error correction and update the check bits correspondingly
// and then write back to IVC side of the register file and pass on to the AES engine

A2_ecc32_incrementer mut (
   .q    (IVCincremented[38:0]), //JL
   .co   (carry_out),
   .inc  (increment),
   .d    (ERRfixed [78:40]) //JLSW
   );

assign   nRF[79:0]   =  {SEC[1],IVCincremented[38:0],SEC[0],ERRfixed[38:0]};

// Generate parity from correct IVC & KEY for use in AES parity preservation
//wire - JLSW Compiler did not like this syntax so flattened it below
  // KEYp[3:0]  = {~^nRF[31:24], ~^nRF[23:16], ~^nRF[15:08], ~^nRF[07:00]},
  // IVCp[3:0]  = {~^nRF[71:64], ~^nRF[63:56], ~^nRF[55:48], ~^nRF[47:40]};

wire   KEYp_jtest3,KEYp_jtest2,KEYp_jtest1,KEYp_jtest0;
assign KEYp_jtest3 = ~^nRF[31:24];
assign KEYp_jtest2 = ~^nRF[23:16];
assign KEYp_jtest1 = ~^nRF[15:08];
assign KEYp_jtest0 = ~^nRF[07:00];

wire   IVCp_jtest3,IVCp_jtest2,IVCp_jtest1,IVCp_jtest0;
assign IVCp_jtest3 = ~^nRF[71:64];
assign IVCp_jtest2 = ~^nRF[63:56];
assign IVCp_jtest1 = ~^nRF[55:48];
assign IVCp_jtest0 = ~^nRF[47:40];

wire   KEYp[3:0];
assign KEYp[3] = ~^nRF[31:24];
assign KEYp[2] = ~^nRF[23:16];
assign KEYp[1] = ~^nRF[15:08];
assign KEYp[0] = ~^nRF[07:00];
wire   IVCp[3:0];
assign IVCp[3] = ~^nRF[71:64];
assign IVCp[2] = ~^nRF[63:56];
assign IVCp[1] = ~^nRF[55:48];
assign IVCp[0] = ~^nRF[47:40];

reg   carry;
always @(posedge clock) if (read && increment) carry <= `tCQ carry_out;

// AES Block ====================================================================================================================

//wire [35:0] KEY_Input; //JL
//assign KEY_Input = {KEYp[3:0],nRF[31:0]}; //JL
//wire [35:0] IVC_Input; //JL
//assign IVC_Input = {IVCp[3:0],nRF[71:40]}; //JL

//JLSW - For compiler
wire [35:0] KEYp_combine;
assign KEYp_combine = {KEYp_jtest3,KEYp_jtest2,KEYp_jtest1,KEYp_jtest0,nRF[31:0]};
wire [35:0] IVCp_combine;
assign IVCp_combine = {IVCp_jtest3,IVCp_jtest2,IVCp_jtest1,IVCp_jtest0,nRF[71:40]};
//End JLSW

reg   [143:0]  KEYbuf, IVCbuf; //JLPL - 8_1_25
always @(posedge clock) if (wr_pipe) KEYbuf[143:0] <= `tCQ  {KEYp_combine[35:0], KEYbuf[143:36]};
always @(posedge clock) if (wr_pipe) IVCbuf[143:0] <= `tCQ  {IVCp_combine[35:0], IVCbuf[143:36]}; //JLPL - 8_1_25

assign   { iSKp[15:12], iSK[127:96], iSKp[11:8], iSK[95:64], iSKp[7:4], iSK[63:32], iSKp[3:0], iSK[31:0]} = KEYbuf[143:0], //JLPL - 10_21_25
//assign   { iSKp[15:12], iSK[127:96], iSKp[11:8], iSK[95:64], iSKp[7:4], iSK[63:32], iSKp[3:0], iSK[31:0]} = (request_addr != 4'h8) ?  KEYbuf[143:0] : {4{36'hf00000000}}, //JLPL - 10_21_25
//         {iIVCp[15:12],iIVC[127:96],iIVCp[11:8],iIVC[95:64],iIVCp[7:4],iIVC[63:32],iIVCp[3:0],iIVC[31:0]} = IVCbuf[143:0]; //JLPL - 10_9_25
         {iIVCp[15:12],iIVC[127:96],iIVCp[11:8],iIVC[95:64],iIVCp[7:4],iIVC[63:32],iIVCp[3:0],iIVC[31:0]} = (!request_addr[3] || request_addr[4]) ?  IVCbuf[143:0] : {4{36'hf00000000}}; //JLPL - 10_17_25

// AES Enryption of IV, Counter and Keys
A2_AES128_ENC iAES (
   // Cipher Text output
   .CTo   ({AESop[15:0], AESo[127:0]}),
   .CTor  ( AESor),
   .CTrd  ( AESrd),
   // Plaintext
   .PTi   ({iIVCp[15:0], iIVC[127:0]}),
   // Key
   .KYi   ({iSKp [15:0], iSK [127:0]}),
   // Plaintext & Key controls
   .PKir  ( AESir),
   .PKwr  ( AESwr),
   // Globals
   .error ( AESerror),  // out
   .abort ( CSR[1]  ),  // in       SW must set and clr this bit
   .reset ( reset),
   .clock ( clock)
   );

// Requests in ------------------------------------------------------------------------------------------------------------------

wire ipmA_rdy; //JLPL - 10_11_25
wire ipmB_rdy; //JLPL - 10_11_25

assign nmosi_fb = ipmA_rdy & CSR[0]; //JLPL - 10_11_25 - pipe_mux21 input is not ready until AES is enabled
assign gmosi_fb = ipmB_rdy & CSR[0]; //JLPL - 10_11_25 - pipe_mux21 input is not ready until AES is enabled 

A2_pipe_mux21 #(5) ipm (            //JLPL - 10_17_25
   .por  (request_or       ), // ordy out
   .pdo  (request_addr[4:0]), // data out //JLPL - 10_17_25
   .prd  (request_rd       ), // read in

   .pirA (ipmA_rdy         ), // irdy  out //JLPL - 10_11_25
   .pdiA (nmosi[4:0]       ), // NEWS request in //JLPL - 10_17_25
   .pwrA (nmosi[5] & CSR[0]), // write in (give) //JLPL - 10_10_25 //JLPL - 10_17_25

   .pirB (ipmB_rdy         ), // irdy  out //JLPL - 10_11_25
   .pdiB (gmosi[4:0]       ), // GCM  request in //JLPL - 10_17_25
   .pwrB (gmosi[5] & CSR[0]), // write in (give) //JLPL - 10_10_25 //JLPL - 10_17_25

   .psel (sGCM             ), // 'B' selected out.  select GCM if true
   .clock(clock            ), // in
   .reset(reset            )  // in
   );

// State Machine for Register Accesses ------------------------------------------------------------------------------------------
// 1. This FSM reads from the input request pipes
// 2. It then reads from the IVC and KEY RegisterFiles in a 4 word burst.
// 3. This data is fed into 2 x 1:4Pipes.  This generates 2 x 128-bit values fed to the AES module (which is also a pipe)
// 4. When reading the register files ECC errors and Counter Increments are handled in line.  For low bandwidth this is of little
//    consequence.  For higher bandwidth this is automatically done in parallel to the AES execution so no harm no foul!!
// GCM commands
// 0: H  = selected Key with 'IVC' of all zeros
// 1: E  = selected Key with 'IV' and Counter +1

reg   [2:0] FSM;  localparam [2:0]  IDLE = 0, READ = 1, CINC = 2, DONE = 3, SECW = 4, XAES = 5, FAIL = 6;
//reg   [3:0] Cycle; //JLSW - Move earlier in code

wire  [3:0] CycleMinus1   = Cycle - 4'h1;      // Done here to discard borrow out before concatenation! //JLPL - 10_15_25

always @(posedge clock or posedge reset)
   if (reset)                                {FSM,Cycle,Cmnd}     <= `tCQ {IDLE, 4'd0, 1'b0        }; //JLPL - 10_18_25
   else case (FSM)
      IDLE  :  if ( CSR[0] && request_or  )  {FSM,Cycle,Cmnd}     <= `tCQ {READ, 4'd3, request_addr[4]}; //JLPL - 10_18_25
      // Read IVC and Key Register Files
      READ  :  if ( KEYded | IVCded       )   FSM                 <= `tCQ  FAIL;
         else  if ( KEYsec | IVCsec       )   FSM                 <= `tCQ  SECW;
         else  if ( Cycle == 4'h0         )   FSM                 <= `tCQ  DONE;
         else  if ( increment             )   FSM                 <= `tCQ  CINC;
         else                                {FSM,Cycle     }     <= `tCQ {READ, CycleMinus1       };
      // Update Counter
      CINC  :                                {FSM,Cycle     }     <= `tCQ {READ, CycleMinus1       };
      // Single Error correction Write
      SECW  :  if ( Cycle == 4'h0         )   FSM                 <= `tCQ  DONE;
         else                                {FSM,Cycle     }     <= `tCQ {READ, CycleMinus1       };
      //
      DONE  :                                 FSM                 <= `tCQ  XAES;
      // pass to AES algorithm
      XAES  :  if ( AESir && DESTir       )   FSM                 <= `tCQ  IDLE;
      // Double Error detect - FAIL
      default  if ( CSR[1]                )  {FSM,Cycle,Cmnd}     <= `tCQ {IDLE, 4'd0, 1'b0        };    // Rollback done
      endcase

assign
   increment   = (Cmnd | CSR[2]) & ((Cycle == 4'd3)  | (Cycle == 4'd2) & carry),      // increment IVC counter (only for Ez/En, not H)! //JLPL - 10_17_25 //JLPL - 10_2-_25 - Add CSR[2] term per Roger
   update      = (FSM == SECW) | (FSM == CINC),
   read        = (FSM == READ),
   idle        = (FSM == IDLE),
   AESwr       = (FSM == XAES),
   request_rd  =  AESwr & AESir & DESTir,
   fail        = (FSM == FAIL);

always @(posedge clock)
   if (reset)  wr_pipe <= `tCQ 1'b0;
   else        wr_pipe <= `tCQ read;   // Read the registerfile and write the output to the pipes

// Destination address of AES Encrypted Block
A2_pipe #(4) i_pa1 (
   .por  (da1_ordy                  ),   // ordy  out
   .pdo  (da1[3:0]                  ),   // data  out
   .prd  (da1_read                  ),   // read  in

   .pir  (DESTir                    ),   // irdy  out
   .pdi  ({sGCM,request_addr[2:0]}  ),   // data  in
   .pwr  (idle & CSR[0] & request_or),   // write in

   .clock(clock                     ),   // in
   .reset(reset                     )    // in
   );

A2_pipe #(4) i_pa2 (
   .por  (dest_ordy               ),   // ordy  out
   .pdo  (ECBad[3:0]              ),   // data  out
   .prd  (ECBlast &&  ECBor &&         //JLPL - 10_10_25 per Roger's GCM interface debugging
         (ECBad[3] ? amiso_rd[1]
                   : amiso_rd[0]) ),   // read  in

   .pir  (da1_read                ),   // irdy  out
   .pdi  (da1[3:0]                ),   // data  in
   .pwr  (da1_ordy                ),   // write in

   .clock(clock                   ),   // in
   .reset(reset                   )    // in
   );

// Encrypted Cipher Block (ECB) of IV, Counter and Keys. Piped to NEWS and GCM

A2_pipe_Nto1L #(36,4,0) i_4T1 ( //JLSW - Little Endian Default //JLPL - 8_1_25 new module ... L
   .por  ( ECB_ordy                 ),
   .pdo  ({ECBpo[3:0],ECBo[31:0]}   ),
   .prd  ( ECBad[3]  ? amiso_rd[1]        // GCM
                     : amiso_rd[0]  ),    // NEWS
   .pir  (AESrd                     ),
   .pdi  ({AESop[15:12],AESo[127:96],AESop[11:8],AESo[95:64],AESop[7:4],AESo[63:32],AESop[3:0],AESo[31:0]} ),
   .pwr  (AESor                     ),

   .plast(ECBlast                   ), //JL //JLPL - 8_1_25 new module ... L
   .clock(clock                     ),
   .reset(reset                     )
   );

// Collate amiso
assign   ECBor =  ECB_ordy & dest_ordy; //JL
//assign   amiso = {ECBor, ECBad[3:0], ECBpo[3:0], ECBo[31:0]}; //JL

endmodule

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
`ifdef   verify_AES_CoP

module   t;

wire  [41:0]   amiso;         // AES output           42 {ECBor, ECBad[4:0], ECBpo[3:0], ECB[31:0]}
wire  [31:0]   imiso;         // Slave data out       32 {                IOk, IOo[31:0]}
wire           nmosi_fb;      // Input ready           1 {AESir}
wire           gmosi_fb;      // Input ready           1 {AESir}
wire           flag;          // flag A2S: error      Double-bit error in ECC for IVC and KEY stores

reg   [47:0]   rmosi;         // Root-of-Trust        48 {RoTload, RoTaddr[7:0], RoTchk[6:0], RoTdata[31:0]}
reg   [47:0]   imosi;         // Slave data in        48 {cIO[2:0], iIO[31:0], aIO[12:0]}
reg   [ 4:0]   nmosi;         // NEWS request/grant,   4 {AESwr, AESreq[3:0]}
reg   [ 4:0]   gmosi;         // Read and consume      4 {AESwr, AESreq[3:0]}
reg   [ 1:0]   amiso_rd;      // Read / Take           2 {ECBrd[1:0]} bit0 = NEWS, bit1 = GCM
reg            reset;
reg            clock;


A2_AES_CoP #(
   .BASE    ( 13'h00000 ),
   .NUM_REGS( 256 )
   ) mut (
   .amiso   (amiso   ),
   .imiso   (imiso   ),
   .nmosi_fb(nmosi_fb),
   .gmosi_fb(gmosi_fb),
   .flag    (flag    ),

   .rmosi   (rmosi   ),
   .imosi   (imosi   ),
   .nmosi   (nmosi   ),
   .gmosi   (gmosi   ),
   .amiso_rd(amiso_rd),
   .reset   (reset   ),
   .clock   (clock   )
   );

initial begin
   rmosi    =  0;
   imosi    =  0;
   nmosi    =  0;
   gmosi    =  0;
   amiso_rd =  0;
   reset    =  0;
   clock    =  0;
   end

endmodule
`endif
